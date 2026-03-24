mod config;
mod display;
mod handler;
mod ipc;
mod tls;

use anyhow::{Context, Result};
use clap::Parser;
use config::{Cli, ServerConfig};
use display::MacDisplay;
use handler::MacInputHandler;
use ironrdp_server::{Credentials, RdpServer, TlsIdentityCtx, gfx::GfxState};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = ServerConfig::load(&cli)?;

    // Initialize logging — write to both stderr and file
    let log_level = config.log_level.as_deref().unwrap_or("info");
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(log_level));

    let log_path = std::env::current_dir().unwrap_or_default().join("macrdp.log");
    let log_file = std::fs::File::create(&log_path)?;

    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;

    tracing_subscriber::registry()
        .with(env_filter)
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(std::io::stderr)
        )
        .with(
            tracing_subscriber::fmt::layer()
                .with_ansi(false)
                .with_writer(std::sync::Mutex::new(log_file))
        )
        .init();

    eprintln!("Log file: {}", log_path.display());

    tracing::info!(?config, "macrdp server starting");

    // Ensure TLS certificates exist
    let (cert_path, key_path) =
        tls::ensure_tls_files(config.cert_path.as_deref(), config.key_path.as_deref())?;

    // Load TLS identity
    let tls_identity = TlsIdentityCtx::init_from_paths(&cert_path, &key_path)
        .context("Failed to load TLS certificate")?;
    let tls_acceptor = tls_identity
        .make_acceptor()
        .context("Failed to create TLS acceptor")?;

    // Check and request required macOS permissions
    check_permissions();

    // Always detect macOS logical display size (needed for mouse coordinate mapping)
    let (logical_w, logical_h) = match macrdp_capture::detect_display_size() {
        Ok((w, h)) => {
            tracing::info!(width = w, height = h, "Detected macOS logical display size");
            (w as u16, h as u16)
        }
        Err(e) => {
            tracing::warn!("Failed to detect display size: {e}, defaulting to 1920x1080");
            (1920u16, 1080u16)
        }
    };

    // Determine RDP desktop resolution (what we capture and send)
    let (width, height) = if config.width > 0 && config.height > 0 {
        (config.width as u16, config.height as u16)
    } else {
        (logical_w, logical_h)
    };

    // Parse quality setting
    let quality = match config.quality.as_deref() {
        Some("low_latency") => macrdp_encode::Quality::LowLatency,
        Some("balanced") => macrdp_encode::Quality::Balanced,
        _ => macrdp_encode::Quality::HighQuality, // default: best quality
    };

    // Parse encoder preference
    let encoder_pref = macrdp_encode::EncoderPreference::from_str_opt(config.encoder.as_deref());
    tracing::info!(?encoder_pref, "Encoder preference");

    // Parse chroma mode
    let mode_444 = config.chroma_mode.as_deref() == Some("avc444");
    tracing::info!(chroma_mode = config.chroma_mode.as_deref().unwrap_or("avc420"), mode_444, "Chroma mode");

    // HiDPI scale: multiply resolution for sharper capture on Retina displays
    let hidpi_scale = config.hidpi_scale.unwrap_or(1).max(1).min(4);
    let (width, height) = (width * hidpi_scale as u16, height * hidpi_scale as u16);
    if hidpi_scale > 1 {
        tracing::info!(hidpi_scale, width, height, "HiDPI scaling enabled");
    }

    // Mouse coordinate mapping: RDP desktop coords → macOS logical coords
    // Auto-computed from the ratio, works correctly for all combinations:
    //   - hidpi_scale=2 on 1080p logical → RDP 3840x2160, mouse ÷2 → 1920x1080
    //   - width=3840 on 1080p logical (no hidpi_scale) → mouse ÷2 → 1920x1080
    //   - width=1920 on 1080p logical → mouse ÷1 → 1920x1080
    let mouse_scale_x = width as f64 / logical_w as f64;
    let mouse_scale_y = height as f64 / logical_h as f64;
    tracing::info!(
        rdp_w = width, rdp_h = height,
        logical_w, logical_h,
        mouse_scale_x = format!("{:.2}", mouse_scale_x),
        mouse_scale_y = format!("{:.2}", mouse_scale_y),
        "Display resolution configured"
    );

    // Create shared GFX state
    let gfx_state = Arc::new(Mutex::new(GfxState::new(width, height, mode_444)));

    // Create input handler with auto-computed mouse scale
    let input_handler = MacInputHandler::new(mouse_scale_x, mouse_scale_y);

    // fixed_resolution = true when user explicitly set width/height in config
    let fixed_resolution = config.width > 0 && config.height > 0;

    // Bitrate override: convert Mbps to bps, or None for auto-calculate
    let bitrate_override = config.bitrate_mbps.map(|mbps| mbps * 1_000_000);

    // Create display with shared GFX state
    let display = MacDisplay::new(width, height, fixed_resolution, config.frame_rate, quality, encoder_pref, mode_444, bitrate_override, Arc::clone(&gfx_state));

    let bind_addr: SocketAddr = format!("0.0.0.0:{}", config.port).parse()?;

    // Build RDP server with Hybrid security (NLA/CredSSP)
    // This enables Windows mstsc to prompt for credentials before connecting
    let mut server = RdpServer::builder()
        .with_addr(bind_addr)
        .with_hybrid(tls_acceptor, tls_identity.pub_key)
        .with_input_handler(input_handler)
        .with_display_handler(display)
        .build();

    // Share GFX state with the server
    server.set_gfx_state(Arc::clone(&gfx_state));

    // Set credentials — required for RDP authentication
    let (username, password) = match (&config.username, &config.password) {
        (Some(u), Some(p)) => (u.clone(), p.clone()),
        _ => {
            // Generate a random password from PID + timestamp entropy
            let seed = std::process::id() as u64
                ^ std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos() as u64;
            let random_pass: String = (0..8)
                .map(|i| {
                    let v = ((seed.wrapping_mul(6364136223846793005).wrapping_add(i * 1442695040888963407)) >> (i * 5 + 3)) % 36;
                    if v < 10 { (b'0' + v as u8) as char } else { (b'a' + (v - 10) as u8) as char }
                })
                .collect();
            let user = "macrdp".to_string();
            tracing::warn!("No credentials in config — using generated credentials:");
            println!("\n  ┌──────────────────────────────────┐");
            println!("  │  Username: {:<22}│", &user);
            println!("  │  Password: {:<22}│", &random_pass);
            println!("  └──────────────────────────────────┘\n");
            tracing::info!("Set [username] and [password] in ~/.config/macrdp/config.toml to use fixed credentials");
            (user, random_pass)
        }
    };
    server.set_credentials(Some(Credentials {
        username: username.clone(),
        password: password.clone(),
        domain: None,
    }));
    tracing::info!("Authentication configured for user: {}", username);

    // Shutdown channel
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::mpsc::channel::<()>(1);

    // Runtime config (hot-updatable via IPC)
    let runtime_config = Arc::new(tokio::sync::Mutex::new(
        ipc::RuntimeConfig::from_server_config(&config),
    ));

    // IPC server (optional — only when --ipc-socket is provided)
    let ipc_socket_path = cli.ipc_socket.clone();
    if let Some(ref socket_path) = ipc_socket_path {
        let (ipc_server, _push_rx, mut cmd_rx) = ipc::IpcServer::new(socket_path.clone());
        let push_tx = ipc_server.push_sender();

        // Start IPC listener
        tokio::spawn(async move {
            if let Err(e) = ipc_server.run().await {
                tracing::error!("IPC server error: {e}");
            }
        });

        // Start command handler
        let cmd_ctx = ipc::CommandContext {
            gfx_state: Arc::clone(&gfx_state),
            runtime_config: Arc::clone(&runtime_config),
            push_tx: push_tx.clone(),
            shutdown_tx: shutdown_tx.clone(),
        };
        tokio::spawn(async move {
            while let Some(cmd) = cmd_rx.recv().await {
                let response = ipc::handle_ipc_command(&cmd, &cmd_ctx).await;
                // Broadcast response to all connected clients
                let _ = cmd_ctx.push_tx.send(response);
            }
        });

        // Start metrics ticker (1-second interval)
        let metrics_gfx = Arc::clone(&gfx_state);
        let metrics_push_tx = push_tx.clone();
        let metrics_runtime = Arc::clone(&runtime_config);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(1));
            let mut prev_bytes_sent: u64 = 0;
            loop {
                interval.tick().await;
                let (latency_ms, bytes_sent) = {
                    let gfx = metrics_gfx.lock().unwrap();
                    (
                        gfx.rtt_ewma_ms,
                        gfx.total_bytes_sent,
                    )
                };
                let fps = {
                    let rc = metrics_runtime.lock().await;
                    rc.frame_rate
                };
                // Instantaneous bitrate: delta bytes over 1-second tick, converted to kbps
                let delta_bytes = bytes_sent.saturating_sub(prev_bytes_sent);
                prev_bytes_sent = bytes_sent;
                let bitrate_kbps = delta_bytes * 8 / 1000;
                let _ = metrics_push_tx.send(ipc::IpcPush::Metrics {
                    fps,
                    bitrate_kbps,
                    latency_ms,
                    bytes_sent,
                });
            }
        });

        tracing::info!(path = %socket_path.display(), "IPC enabled");
    }

    tracing::info!(%bind_addr, "RDP server listening");
    tracing::info!("Connect using an RDP client (e.g., Windows mstsc or Microsoft Remote Desktop)");

    // Run server with shutdown support
    tokio::select! {
        result = server.run() => {
            result.context("RDP server error")?;
        }
        _ = shutdown_rx.recv() => {
            tracing::info!("Shutdown signal received, stopping server");
        }
    }

    // Clean up IPC socket on exit
    if let Some(ref socket_path) = ipc_socket_path {
        ipc::cleanup_socket(socket_path);
    }

    Ok(())
}

/// Check and request all required macOS permissions at startup.
fn check_permissions() {
    tracing::info!("Checking macOS permissions...");
    let mut all_granted = true;

    // 1. Screen Recording
    if macrdp_capture::check_screen_recording_permission() {
        tracing::info!("[OK] Screen Recording permission granted");
    } else {
        all_granted = false;
        tracing::warn!("[!!] Screen Recording permission NOT granted");
        tracing::warn!("     Requesting Screen Recording access...");
        macrdp_capture::request_screen_recording_permission();
        // Check again after request
        if !macrdp_capture::check_screen_recording_permission() {
            tracing::error!(
                "     Screen Recording denied. Screen capture will fail."
            );
            tracing::error!(
                "     Go to: System Settings > Privacy & Security > Screen Recording"
            );
            tracing::error!(
                "     Add this app, then RESTART the server."
            );
            macrdp_capture::open_screen_recording_settings();
        }
    }

    // 2. Accessibility (required for keyboard/mouse injection)
    if macrdp_input::check_accessibility_permission() {
        tracing::info!("[OK] Accessibility permission granted");
    } else {
        all_granted = false;
        tracing::warn!("[!!] Accessibility permission NOT granted");
        tracing::warn!("     Requesting Accessibility access...");
        // This will show the system prompt dialog
        macrdp_input::request_accessibility_permission();

        if !macrdp_input::check_accessibility_permission() {
            tracing::error!(
                "     Accessibility denied. Keyboard/mouse input will NOT work."
            );
            tracing::error!(
                "     Go to: System Settings > Privacy & Security > Accessibility"
            );
            tracing::error!(
                "     Add this app, then RESTART the server."
            );
            macrdp_input::open_accessibility_settings();
        }
    }

    if all_granted {
        tracing::info!("All permissions granted");
    } else {
        tracing::warn!(
            "Some permissions missing — server will start but may have limited functionality."
        );
        tracing::warn!(
            "After granting permissions in System Settings, RESTART the server."
        );
    }
}
