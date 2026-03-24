//! RDP server lifecycle: start, stop, metrics, connections

use std::net::SocketAddr;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::time::Instant;

use anyhow::{Context, Result};
use ironrdp_server::gfx::GfxState;
use tokio::sync::Notify;
use tokio::task::JoinHandle;

use crate::callbacks::*;
use crate::config::ServerConfig;
use crate::permissions;
use crate::tls;

/// Handle to a running RDP server. Use to query state and stop the server.
pub struct ServerHandle {
    port: u16,
    started_at: Instant,
    gfx_state: Arc<Mutex<GfxState>>,
    shutdown_notify: Arc<Notify>,
    stopped: AtomicBool,
    /// The OS thread running the RDP server (RdpServer is !Send, needs dedicated thread)
    server_thread: Mutex<Option<std::thread::JoinHandle<()>>>,
    metrics_task: Mutex<Option<JoinHandle<()>>>,
}

impl ServerHandle {
    /// The TCP port the server is listening on.
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Current server status.
    pub fn status(&self) -> ServerStatus {
        if self.stopped.load(Ordering::Relaxed) {
            ServerStatus {
                running: false,
                state: "stopped".to_string(),
                uptime_secs: 0,
            }
        } else {
            ServerStatus {
                running: true,
                state: "running".to_string(),
                uptime_secs: self.started_at.elapsed().as_secs(),
            }
        }
    }

    /// Current performance metrics.
    pub fn metrics(&self) -> Metrics {
        let gfx = self.gfx_state.lock().unwrap();
        Metrics {
            fps: 0, // Actual FPS comes from config/runtime
            bitrate_kbps: 0,
            latency_ms: gfx.rtt_ewma_ms,
            bytes_sent: gfx.total_bytes_sent,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
        }
    }

    /// Gracefully stop the server.
    pub async fn stop(&self) -> Result<()> {
        if self.stopped.swap(true, Ordering::Relaxed) {
            return Ok(()); // Already stopped
        }
        self.shutdown_notify.notify_waiters();

        // Wait for server thread to finish
        if let Some(thread) = self.server_thread.lock().unwrap().take() {
            let _ = tokio::task::spawn_blocking(move || {
                let _ = thread.join();
            })
            .await;
        }
        // Cancel metrics task
        if let Some(task) = self.metrics_task.lock().unwrap().take() {
            task.abort();
        }
        Ok(())
    }
}

impl Drop for ServerHandle {
    fn drop(&mut self) {
        if !self.stopped.load(Ordering::Relaxed) {
            self.shutdown_notify.notify_waiters();
        }
    }
}

/// Arguments that are Send-safe, passed to the server thread.
/// RdpServer itself is !Send, so we pass everything needed to construct it.
struct ServerThreadArgs {
    config: ServerConfig,
    bind_addr: SocketAddr,
    cert_path: std::path::PathBuf,
    key_path: std::path::PathBuf,
    #[allow(dead_code)]
    logical_w: u16,
    #[allow(dead_code)]
    logical_h: u16,
    width: u16,
    height: u16,
    quality: macrdp_encode::Quality,
    encoder_pref: macrdp_encode::EncoderPreference,
    mode_444: bool,
    mouse_scale_x: f64,
    mouse_scale_y: f64,
    gfx_state: Arc<Mutex<GfxState>>,
    shutdown_notify: Arc<Notify>,
    handler: Arc<dyn ServerEventHandler>,
}

/// Start the RDP server with the given configuration and event handler.
/// Returns a handle to control the running server.
pub async fn start_server(
    config: ServerConfig,
    handler: impl ServerEventHandler,
) -> Result<Arc<ServerHandle>> {
    let handler: Arc<dyn ServerEventHandler> = Arc::new(handler);

    // Notify handler: starting
    handler.on_status_change(ServerStatus {
        running: true,
        state: "starting".to_string(),
        uptime_secs: 0,
    });

    // Check and request permissions
    let perms = permissions::request_permissions();
    tracing::info!(?perms, "Permission status");

    // TLS
    let (cert_path, key_path) =
        tls::ensure_tls_files(config.cert_path.as_deref(), config.key_path.as_deref())?;

    // Display detection
    let (logical_w, logical_h) = match permissions::detect_display_size() {
        Ok((w, h)) => {
            tracing::info!(width = w, height = h, "Detected macOS logical display size");
            (w as u16, h as u16)
        }
        Err(e) => {
            tracing::warn!("Failed to detect display size: {e}, defaulting to 1920x1080");
            (1920u16, 1080u16)
        }
    };

    // Resolution
    let (width, height) = if config.width > 0 && config.height > 0 {
        (config.width as u16, config.height as u16)
    } else {
        (logical_w, logical_h)
    };

    // Quality / encoder / chroma
    let quality = match config.quality.as_deref() {
        Some("low_latency") => macrdp_encode::Quality::LowLatency,
        Some("balanced") => macrdp_encode::Quality::Balanced,
        _ => macrdp_encode::Quality::HighQuality,
    };
    let encoder_pref = macrdp_encode::EncoderPreference::from_str_opt(config.encoder.as_deref());
    let mode_444 = config.chroma_mode.as_deref() == Some("avc444");

    // HiDPI
    let hidpi_scale = config.hidpi_scale.unwrap_or(1).max(1).min(4);
    let (width, height) = (width * hidpi_scale as u16, height * hidpi_scale as u16);

    // Mouse coordinate mapping
    let mouse_scale_x = width as f64 / logical_w as f64;
    let mouse_scale_y = height as f64 / logical_h as f64;

    // GFX state (shared between server thread and metrics task)
    let gfx_state = Arc::new(Mutex::new(GfxState::new(width, height, mode_444)));

    let bind_addr: SocketAddr = format!("0.0.0.0:{}", config.port).parse()?;
    let shutdown_notify = Arc::new(Notify::new());
    let port = config.port;

    // Pack everything the server thread needs
    let args = ServerThreadArgs {
        config: config.clone(),
        bind_addr,
        cert_path,
        key_path,
        logical_w,
        logical_h,
        width,
        height,
        quality,
        encoder_pref,
        mode_444,
        mouse_scale_x,
        mouse_scale_y,
        gfx_state: Arc::clone(&gfx_state),
        shutdown_notify: Arc::clone(&shutdown_notify),
        handler: Arc::clone(&handler),
    };

    // Spawn RDP server on a dedicated OS thread.
    // RdpServer is !Send (contains dyn SoundServerFactory, dyn CliprdrServerFactory),
    // so we construct and run it entirely within this thread.
    let server_thread = std::thread::Builder::new()
        .name("rdp-server".into())
        .spawn(move || {
            run_server_thread(args);
        })
        .context("Failed to spawn RDP server thread")?;

    // Spawn metrics ticker (1 second interval) — all types here are Send
    let metrics_gfx = Arc::clone(&gfx_state);
    let handler_for_metrics = Arc::clone(&handler);
    let fps = config.frame_rate;
    let metrics_task = tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
        let mut prev_bytes_sent: u64 = 0;
        loop {
            interval.tick().await;
            let (latency_ms, bytes_sent) = {
                let gfx = metrics_gfx.lock().unwrap();
                (gfx.rtt_ewma_ms, gfx.total_bytes_sent)
            };
            let delta_bytes = bytes_sent.saturating_sub(prev_bytes_sent);
            prev_bytes_sent = bytes_sent;
            let bitrate_kbps = delta_bytes * 8 / 1000;
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            handler_for_metrics.on_metrics(Metrics {
                fps,
                bitrate_kbps,
                latency_ms,
                bytes_sent,
                timestamp,
            });
        }
    });

    let handle = Arc::new(ServerHandle {
        port,
        started_at: Instant::now(),
        gfx_state,
        shutdown_notify,
        stopped: AtomicBool::new(false),
        server_thread: Mutex::new(Some(server_thread)),
        metrics_task: Mutex::new(Some(metrics_task)),
    });

    // Notify handler: running
    handler.on_status_change(ServerStatus {
        running: true,
        state: "running".to_string(),
        uptime_secs: 0,
    });

    Ok(handle)
}

/// Runs the RDP server entirely on the current thread. Called from the dedicated
/// server thread. Constructs the !Send RdpServer here so it never crosses thread
/// boundaries.
fn run_server_thread(args: ServerThreadArgs) {
    use crate::display::MacDisplay;
    use crate::handler::MacInputHandler;
    use ironrdp_server::{Credentials, RdpServer, TlsIdentityCtx};

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("Failed to create tokio runtime for RDP server");

    let local = tokio::task::LocalSet::new();
    local.block_on(&rt, async move {
        // Load TLS identity (on this thread)
        let tls_identity = match TlsIdentityCtx::init_from_paths(&args.cert_path, &args.key_path) {
            Ok(id) => id,
            Err(e) => {
                tracing::error!("Failed to load TLS certificate: {e}");
                args.handler.on_status_change(ServerStatus {
                    running: false,
                    state: format!("error: {e}"),
                    uptime_secs: 0,
                });
                return;
            }
        };
        let tls_acceptor = match tls_identity.make_acceptor() {
            Ok(a) => a,
            Err(e) => {
                tracing::error!("Failed to create TLS acceptor: {e}");
                args.handler.on_status_change(ServerStatus {
                    running: false,
                    state: format!("error: {e}"),
                    uptime_secs: 0,
                });
                return;
            }
        };

        // Create input handler
        let input_handler = MacInputHandler::new(args.mouse_scale_x, args.mouse_scale_y);

        // Create display
        let fixed_resolution = args.config.width > 0 && args.config.height > 0;
        let bitrate_override = args.config.bitrate_mbps.map(|mbps| mbps * 1_000_000);
        let display = MacDisplay::new(
            args.width,
            args.height,
            fixed_resolution,
            args.config.frame_rate,
            args.quality,
            args.encoder_pref,
            args.mode_444,
            bitrate_override,
            Arc::clone(&args.gfx_state),
        );

        // Build RDP server (this is the !Send type)
        let mut server = RdpServer::builder()
            .with_addr(args.bind_addr)
            .with_hybrid(tls_acceptor, tls_identity.pub_key)
            .with_input_handler(input_handler)
            .with_display_handler(display)
            .build();

        server.set_gfx_state(Arc::clone(&args.gfx_state));

        // Credentials
        let (username, password) = match (&args.config.username, &args.config.password) {
            (Some(u), Some(p)) => (u.clone(), p.clone()),
            _ => {
                let seed = std::process::id() as u64
                    ^ std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_nanos() as u64;
                let random_pass: String = (0..8)
                    .map(|i| {
                        let v = ((seed
                            .wrapping_mul(6364136223846793005)
                            .wrapping_add(i * 1442695040888963407))
                            >> (i * 5 + 3))
                            % 36;
                        if v < 10 {
                            (b'0' + v as u8) as char
                        } else {
                            (b'a' + (v - 10) as u8) as char
                        }
                    })
                    .collect();
                let user = "macrdp".to_string();
                tracing::warn!("No credentials in config — using generated credentials:");
                println!("\n  ┌──────────────────────────────────┐");
                println!("  │  Username: {:<22}│", &user);
                println!("  │  Password: {:<22}│", &random_pass);
                println!("  └──────────────────────────────────┘\n");
                (user, random_pass)
            }
        };
        server.set_credentials(Some(Credentials {
            username: username.clone(),
            password: password.clone(),
            domain: None,
        }));
        tracing::info!("Authentication configured for user: {}", username);

        tracing::info!(%args.bind_addr, "RDP server listening");
        tokio::select! {
            result = server.run() => {
                if let Err(e) = result {
                    tracing::error!("RDP server error: {e}");
                    args.handler.on_status_change(ServerStatus {
                        running: false,
                        state: format!("error: {e}"),
                        uptime_secs: 0,
                    });
                }
            }
            _ = args.shutdown_notify.notified() => {
                tracing::info!("Shutdown signal received, stopping server");
            }
        }
    });
}
