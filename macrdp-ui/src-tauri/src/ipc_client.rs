use std::path::Path;
use std::sync::Arc;

use serde::Deserialize;
use tauri::AppHandle;
use tauri::Emitter;
use tauri::Manager;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::unix::OwnedWriteHalf;
use tokio::sync::Mutex;

use crate::database::Database;
use crate::state::{AppState, Connection, LogEntry, Metrics, PermissionStatus, ServerStatus};

/// Maximum number of log entries to keep in memory.
const MAX_LOG_ENTRIES: usize = 5000;

/// Messages received from the macrdp server via IPC.
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum IpcMessage {
    #[serde(rename = "status")]
    Status { state: String, uptime_secs: u64 },

    #[serde(rename = "metrics")]
    Metrics {
        fps: u32,
        bitrate_kbps: u32,
        latency_ms: f64,
        bytes_sent: u64,
    },

    #[serde(rename = "connection")]
    Connection {
        event: String,
        client_ip: String,
        client_name: Option<String>,
        #[allow(dead_code)]
        duration_secs: Option<u64>,
        bytes_total: Option<u64>,
    },

    #[serde(rename = "log")]
    Log {
        level: String,
        message: String,
        timestamp: String,
    },

    #[serde(rename = "response")]
    Response {
        id: String,
        success: bool,
        data: Option<serde_json::Value>,
        error: Option<serde_json::Value>,
    },

    #[serde(rename = "permissions")]
    Permissions {
        screen_capture: bool,
        accessibility: bool,
        microphone: bool,
    },
}

/// Client for IPC communication with the running macrdp server.
pub struct IpcClient {
    writer: Arc<Mutex<Option<OwnedWriteHalf>>>,
}

impl IpcClient {
    pub fn new() -> Self {
        Self {
            writer: Arc::new(Mutex::new(None)),
        }
    }

    /// Connect to the macrdp server's Unix socket and start the read loop.
    ///
    /// The read loop parses JSON Lines into typed messages, updates `AppState`,
    /// and emits Tauri events. On disconnect, the writer is set to `None` and
    /// the method returns `Ok(())` — the caller is responsible for reconnection.
    pub async fn connect(
        &self,
        socket_path: &Path,
        app: AppHandle,
    ) -> Result<(), String> {
        let stream = tokio::net::UnixStream::connect(socket_path)
            .await
            .map_err(|e| format!("Failed to connect to IPC socket: {}", e))?;

        let (read_half, write_half) = stream.into_split();

        // Store the writer
        {
            let mut guard = self.writer.lock().await;
            *guard = Some(write_half);
        }

        let writer_ref = self.writer.clone();

        // Spawn the read loop
        tokio::spawn(async move {
            let reader = BufReader::new(read_half);
            let mut lines = reader.lines();

            loop {
                match lines.next_line().await {
                    Ok(Some(line)) => {
                        if line.trim().is_empty() {
                            continue;
                        }
                        if let Err(e) =
                            handle_message(&line, &app).await
                        {
                            tracing::warn!(
                                "Failed to handle IPC message: {}",
                                e
                            );
                        }
                    }
                    Ok(None) => {
                        // EOF — server disconnected
                        tracing::info!("IPC connection closed by server");
                        break;
                    }
                    Err(e) => {
                        tracing::error!("IPC read error: {}", e);
                        break;
                    }
                }
            }

            // Clear writer on disconnect
            let mut guard = writer_ref.lock().await;
            *guard = None;
        });

        Ok(())
    }

    /// Send a command to the server as a JSON line.
    pub async fn send_command(&self, cmd: &str) -> Result<(), String> {
        let mut guard = self.writer.lock().await;
        if let Some(ref mut writer) = *guard {
            let mut data = cmd.to_string();
            if !data.ends_with('\n') {
                data.push('\n');
            }
            writer
                .write_all(data.as_bytes())
                .await
                .map_err(|e| format!("Failed to send IPC command: {}", e))?;
            writer
                .flush()
                .await
                .map_err(|e| format!("Failed to flush IPC command: {}", e))?;
            Ok(())
        } else {
            Err("Not connected to IPC socket".into())
        }
    }
}

/// Process a single IPC message line, update AppState, and emit Tauri events.
async fn handle_message(
    line: &str,
    app: &AppHandle,
) -> Result<(), String> {
    let msg: IpcMessage = serde_json::from_str(line)
        .map_err(|e| format!("Invalid IPC JSON: {} — line: {}", e, line))?;

    let app_state = app
        .try_state::<Arc<AppState>>()
        .ok_or("AppState not available")?;

    match msg {
        IpcMessage::Status { state, uptime_secs } => {
            let running = state != "stopped";
            let status = ServerStatus {
                running,
                state: state.clone(),
                uptime_secs,
                pid: None,
            };
            {
                let mut guard = app_state.status.lock().await;
                *guard = status.clone();
            }
            let _ = app.emit("server-status", &status);
        }

        IpcMessage::Metrics {
            fps,
            bitrate_kbps,
            latency_ms,
            bytes_sent,
        } => {
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let metrics = Metrics {
                fps,
                bitrate_kbps,
                latency_ms,
                bytes_sent,
                timestamp,
            };
            {
                let mut guard = app_state.metrics.lock().await;
                *guard = metrics.clone();
            }
            let _ = app.emit("metrics", &metrics);

            // Update tray icon text with connection stats
            crate::tray::update_tray_status(
                app,
                &app_state.status.lock().await.state,
                Some(bitrate_kbps),
                Some(latency_ms),
            );
        }

        IpcMessage::Connection {
            event,
            client_ip,
            client_name,
            bytes_total,
            ..
        } => {
            match event.as_str() {
                "connected" => {
                    let conn = Connection {
                        client_ip: client_ip.clone(),
                        client_name: client_name
                            .unwrap_or_else(|| "Unknown".to_string()),
                        connected_at: chrono::Utc::now().to_rfc3339(),
                        bytes_total: bytes_total.unwrap_or(0),
                    };
                    let mut guard = app_state.connections.lock().await;
                    guard.push(conn);
                }
                "disconnected" => {
                    let mut guard = app_state.connections.lock().await;
                    guard.retain(|c| c.client_ip != client_ip);

                    // Write to SQLite database if available
                    if let Some(db) = app.try_state::<Arc<Database>>() {
                        let _ = db.record_disconnection(
                            &client_ip,
                            client_name.as_deref(),
                            bytes_total.unwrap_or(0),
                        );
                    }
                }
                other => {
                    tracing::warn!(
                        "Unknown connection event: {}",
                        other
                    );
                }
            }
            let connections = app_state.connections.lock().await.clone();
            let _ = app.emit("connections", &connections);
        }

        IpcMessage::Log {
            level,
            message,
            timestamp,
        } => {
            let entry = LogEntry {
                level,
                message,
                timestamp,
            };
            {
                let mut guard = app_state.logs.lock().await;
                if guard.len() >= MAX_LOG_ENTRIES {
                    guard.pop_front();
                }
                guard.push_back(entry.clone());
            }
            let _ = app.emit("log", &entry);
        }

        IpcMessage::Response {
            id,
            success,
            data,
            error,
        } => {
            let payload = serde_json::json!({
                "id": id,
                "success": success,
                "data": data,
                "error": error,
            });
            let _ = app.emit("ipc-response", &payload);
        }

        IpcMessage::Permissions {
            screen_capture,
            accessibility,
            microphone,
        } => {
            let perms = PermissionStatus {
                screen_capture,
                accessibility,
                microphone,
            };
            {
                let mut guard = app_state.permissions.lock().await;
                *guard = perms.clone();
            }
            let _ = app.emit("permissions", &perms);
        }
    }

    Ok(())
}
