use anyhow::Result;
use clap::Parser;
use macrdp_core::{
    ConnectionEvent, LogEntry, Metrics, ServerConfig, ServerEventHandler, ServerStatus,
};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "macrdp", about = "macOS RDP Server")]
struct Cli {
    /// TCP port to listen on
    #[arg(short, long)]
    port: Option<u16>,

    /// Path to config file
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Target frame rate (30, 60, or 120)
    #[arg(long)]
    frame_rate: Option<u32>,

    /// Log level: trace, debug, info, warn, error
    #[arg(long)]
    log_level: Option<String>,
}

/// CLI event handler — logs all server events via tracing.
struct CliEventHandler;

impl ServerEventHandler for CliEventHandler {
    fn on_status_change(&self, status: ServerStatus) {
        tracing::info!(
            running = status.running,
            state = %status.state,
            uptime_secs = status.uptime_secs,
            "Server status changed"
        );
    }

    fn on_metrics(&self, metrics: Metrics) {
        tracing::debug!(
            fps = metrics.fps,
            bitrate_kbps = metrics.bitrate_kbps,
            latency_ms = format!("{:.1}", metrics.latency_ms),
            bytes_sent = metrics.bytes_sent,
            "Metrics"
        );
    }

    fn on_connection(&self, event: ConnectionEvent) {
        match event {
            ConnectionEvent::Connected(info) => {
                tracing::info!(
                    client_ip = %info.client_ip,
                    client_name = %info.client_name,
                    "Client connected"
                );
            }
            ConnectionEvent::Disconnected(info) => {
                tracing::info!(
                    client_ip = %info.client_ip,
                    client_name = %info.client_name,
                    duration_secs = info.duration_secs,
                    bytes_total = info.bytes_total,
                    "Client disconnected"
                );
            }
        }
    }

    fn on_log(&self, entry: LogEntry) {
        match entry.level.as_str() {
            "error" => tracing::error!("{}", entry.message),
            "warn" => tracing::warn!("{}", entry.message),
            "debug" => tracing::debug!("{}", entry.message),
            "trace" => tracing::trace!("{}", entry.message),
            _ => tracing::info!("{}", entry.message),
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Load config from file (or defaults)
    let mut config = ServerConfig::load_from_file(cli.config.as_deref())?;

    // Apply CLI overrides
    if let Some(port) = cli.port {
        config.port = port;
    }
    if let Some(fps) = cli.frame_rate {
        config.frame_rate = fps;
    }
    if let Some(level) = &cli.log_level {
        config.log_level = Some(level.clone());
    }

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
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stderr))
        .with(
            tracing_subscriber::fmt::layer()
                .with_ansi(false)
                .with_writer(std::sync::Mutex::new(log_file)),
        )
        .init();

    eprintln!("Log file: {}", log_path.display());
    tracing::info!(?config, "macrdp server starting");

    // Start the server
    let handle = macrdp_core::start_server(config, CliEventHandler).await?;

    tracing::info!(port = handle.port(), "RDP server listening — press Ctrl+C to stop");

    // Wait for Ctrl+C
    tokio::signal::ctrl_c().await?;
    tracing::info!("Ctrl+C received, shutting down...");

    handle.stop().await?;
    tracing::info!("Server stopped");

    Ok(())
}
