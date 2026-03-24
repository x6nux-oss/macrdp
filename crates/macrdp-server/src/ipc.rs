//! IPC module: Unix Domain Socket + JSON Lines protocol for Tauri UI communication.
//!
//! When `--ipc-socket <path>` is passed, the server listens on a Unix socket.
//! Each connected client exchanges newline-delimited JSON messages.
//!
//! Protocol:
//! - UI → CLI: `IpcCommand` (JSON Lines)
//! - CLI → UI: `IpcPush` (JSON Lines, broadcast to all connected clients)

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixListener;
use tokio::sync::{broadcast, mpsc};

// ── Message types ───────────────────────────────────────────────────────────

/// CLI → UI push messages (broadcast to all connected clients)
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", content = "data")]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)] // Variants are constructed dynamically / used by the Tauri UI side
pub enum IpcPush {
    /// Server status update
    Status {
        state: String,
        uptime_secs: Option<u64>,
        client_ip: Option<String>,
    },
    /// Performance metrics (sent periodically)
    Metrics {
        fps: u32,
        bitrate_kbps: u64,
        latency_ms: f64,
        bytes_sent: u64,
    },
    /// Client connection event
    Connection { event: String, client_ip: String },
    /// Log message forwarding
    Log {
        level: String,
        message: String,
    },
    /// Response to a command
    Response {
        id: String,
        success: bool,
        #[serde(skip_serializing_if = "Option::is_none")]
        data: Option<serde_json::Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<IpcError>,
    },
    /// Permission status
    Permissions {
        screen_recording: bool,
        accessibility: bool,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcError {
    pub code: String,
    pub message: String,
}

/// UI → CLI command (one per JSON line)
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)] // Fields are deserialized from JSON sent by the Tauri UI side
pub struct IpcCommand {
    pub id: String,
    pub cmd: String,
    #[serde(default)]
    pub key: Option<String>,
    #[serde(default)]
    pub value: Option<serde_json::Value>,
    #[serde(default)]
    pub since: Option<String>,
    #[serde(default)]
    pub limit: Option<u32>,
}

/// Hot-updatable runtime config fields
#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    pub frame_rate: u32,
    pub bitrate_kbps: Option<u32>,
    pub log_level: String,
}

impl RuntimeConfig {
    pub fn from_server_config(config: &crate::config::ServerConfig) -> Self {
        Self {
            frame_rate: config.frame_rate,
            bitrate_kbps: config.bitrate_mbps.map(|mbps| mbps * 1000),
            log_level: config
                .log_level
                .clone()
                .unwrap_or_else(|| "info".to_string()),
        }
    }
}

// ── IPC Server ──────────────────────────────────────────────────────────────

pub struct IpcServer {
    socket_path: PathBuf,
    push_tx: broadcast::Sender<IpcPush>,
    cmd_tx: mpsc::Sender<IpcCommand>,
}

impl IpcServer {
    /// Create IPC server. Returns (server, push_rx, cmd_rx).
    /// - `push_rx`: subscribe to push messages (for monitoring / testing)
    /// - `cmd_rx`: receive parsed commands from connected clients
    pub fn new(
        socket_path: PathBuf,
    ) -> (Self, broadcast::Receiver<IpcPush>, mpsc::Receiver<IpcCommand>) {
        let (push_tx, push_rx) = broadcast::channel::<IpcPush>(256);
        let (cmd_tx, cmd_rx) = mpsc::channel::<IpcCommand>(64);
        let server = Self {
            socket_path,
            push_tx,
            cmd_tx,
        };
        (server, push_rx, cmd_rx)
    }

    /// Get a clone of the push sender (for sending pushes from other tasks)
    pub fn push_sender(&self) -> broadcast::Sender<IpcPush> {
        self.push_tx.clone()
    }

    /// Run the IPC listener. Spawns a task per connected client.
    pub async fn run(self) -> anyhow::Result<()> {
        // Clean up stale socket file
        cleanup_socket(&self.socket_path);

        let listener = UnixListener::bind(&self.socket_path)?;
        tracing::info!(path = %self.socket_path.display(), "IPC server listening");

        loop {
            match listener.accept().await {
                Ok((stream, _addr)) => {
                    tracing::debug!("IPC client connected");
                    let push_rx = self.push_tx.subscribe();
                    let cmd_tx = self.cmd_tx.clone();
                    tokio::spawn(handle_client(stream, push_rx, cmd_tx));
                }
                Err(e) => {
                    tracing::warn!("IPC accept error: {e}");
                }
            }
        }
    }
}

/// Handle a single IPC client connection.
async fn handle_client(
    stream: tokio::net::UnixStream,
    mut push_rx: broadcast::Receiver<IpcPush>,
    cmd_tx: mpsc::Sender<IpcCommand>,
) {
    let (reader, mut writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();

    loop {
        tokio::select! {
            // Read commands from client
            line_result = lines.next_line() => {
                match line_result {
                    Ok(Some(line)) => {
                        let line = line.trim().to_string();
                        if line.is_empty() {
                            continue;
                        }
                        match serde_json::from_str::<IpcCommand>(&line) {
                            Ok(cmd) => {
                                if cmd_tx.send(cmd).await.is_err() {
                                    tracing::debug!("IPC command channel closed");
                                    break;
                                }
                            }
                            Err(e) => {
                                tracing::warn!("IPC parse error: {e}, line: {line}");
                            }
                        }
                    }
                    Ok(None) => {
                        // Client disconnected
                        tracing::debug!("IPC client disconnected");
                        break;
                    }
                    Err(e) => {
                        tracing::warn!("IPC read error: {e}");
                        break;
                    }
                }
            }
            // Forward push messages to client
            push_result = push_rx.recv() => {
                match push_result {
                    Ok(msg) => {
                        match serde_json::to_string(&msg) {
                            Ok(json) => {
                                let line = format!("{json}\n");
                                if writer.write_all(line.as_bytes()).await.is_err() {
                                    tracing::debug!("IPC write failed, client gone");
                                    break;
                                }
                            }
                            Err(e) => {
                                tracing::warn!("IPC serialize error: {e}");
                            }
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("IPC client lagged by {n} messages");
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        break;
                    }
                }
            }
        }
    }
}

// ── Command handler ─────────────────────────────────────────────────────────

/// Context for handling IPC commands
pub struct CommandContext {
    pub gfx_state: Arc<std::sync::Mutex<ironrdp_server::gfx::GfxState>>,
    pub runtime_config: Arc<tokio::sync::Mutex<RuntimeConfig>>,
    pub push_tx: broadcast::Sender<IpcPush>,
    pub shutdown_tx: mpsc::Sender<()>,
}

/// Handle a single IPC command, returning the response push.
pub async fn handle_ipc_command(cmd: &IpcCommand, ctx: &CommandContext) -> IpcPush {
    match cmd.cmd.as_str() {
        "get_status" => {
            let gfx = ctx.gfx_state.lock().unwrap();
            let uptime_secs = gfx.start_time.map(|t| t.elapsed().as_secs());
            let state = if gfx.is_ready() {
                "running"
            } else {
                "waiting"
            };
            IpcPush::Response {
                id: cmd.id.clone(),
                success: true,
                data: Some(serde_json::json!({
                    "state": state,
                    "uptime_secs": uptime_secs,
                })),
                error: None,
            }
        }

        "get_permissions" => {
            let screen = macrdp_capture::check_screen_recording_permission();
            let accessibility = macrdp_input::check_accessibility_permission();
            IpcPush::Response {
                id: cmd.id.clone(),
                success: true,
                data: Some(serde_json::json!({
                    "screen_recording": screen,
                    "accessibility": accessibility,
                })),
                error: None,
            }
        }

        "set_config" => {
            let key = match &cmd.key {
                Some(k) => k.as_str(),
                None => {
                    return make_error_response(&cmd.id, "MISSING_KEY", "key is required");
                }
            };
            let value = match &cmd.value {
                Some(v) => v,
                None => {
                    return make_error_response(&cmd.id, "MISSING_VALUE", "value is required");
                }
            };

            // Hot-updatable keys
            match key {
                "fps" => {
                    if let Some(fps) = value.as_u64() {
                        let mut rc = ctx.runtime_config.lock().await;
                        rc.frame_rate = fps as u32;
                        tracing::info!(fps, "IPC: fps updated");
                        make_ok_response(&cmd.id)
                    } else {
                        make_error_response(&cmd.id, "INVALID_VALUE", "fps must be u32")
                    }
                }
                "bitrate_kbps" => {
                    if let Some(kbps) = value.as_u64() {
                        let mut rc = ctx.runtime_config.lock().await;
                        rc.bitrate_kbps = Some(kbps as u32);
                        tracing::info!(kbps, "IPC: bitrate_kbps updated");
                        make_ok_response(&cmd.id)
                    } else {
                        make_error_response(
                            &cmd.id,
                            "INVALID_VALUE",
                            "bitrate_kbps must be u32",
                        )
                    }
                }
                "log_level" => {
                    if let Some(level) = value.as_str() {
                        let mut rc = ctx.runtime_config.lock().await;
                        rc.log_level = level.to_string();
                        tracing::info!(level, "IPC: log_level updated");
                        make_ok_response(&cmd.id)
                    } else {
                        make_error_response(
                            &cmd.id,
                            "INVALID_VALUE",
                            "log_level must be a string",
                        )
                    }
                }
                // Keys that require restart
                "port" | "bind_address" | "resolution_width" | "resolution_height"
                | "max_connections" | "tls_cert_path" | "tls_key_path" => IpcPush::Response {
                    id: cmd.id.clone(),
                    success: true,
                    data: Some(serde_json::json!({
                        "restart_required": true,
                    })),
                    error: None,
                },
                _ => make_error_response(&cmd.id, "UNKNOWN_KEY", &format!("unknown key: {key}")),
            }
        }

        "get_logs" => {
            // Logs are handled by tracing; return empty array
            IpcPush::Response {
                id: cmd.id.clone(),
                success: true,
                data: Some(serde_json::json!({"logs": []})),
                error: None,
            }
        }

        "shutdown" => {
            tracing::info!("IPC: shutdown requested");
            let _ = ctx.shutdown_tx.send(()).await;
            IpcPush::Response {
                id: cmd.id.clone(),
                success: true,
                data: None,
                error: None,
            }
        }

        _ => make_error_response(
            &cmd.id,
            "UNKNOWN_CMD",
            &format!("unknown command: {}", cmd.cmd),
        ),
    }
}

fn make_ok_response(id: &str) -> IpcPush {
    IpcPush::Response {
        id: id.to_string(),
        success: true,
        data: None,
        error: None,
    }
}

fn make_error_response(id: &str, code: &str, message: &str) -> IpcPush {
    IpcPush::Response {
        id: id.to_string(),
        success: false,
        data: None,
        error: Some(IpcError {
            code: code.to_string(),
            message: message.to_string(),
        }),
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────────

/// Remove stale socket file if it exists
pub fn cleanup_socket(path: &Path) {
    if path.exists() {
        if let Err(e) = std::fs::remove_file(path) {
            tracing::warn!("Failed to remove stale socket {}: {e}", path.display());
        }
    }
}
