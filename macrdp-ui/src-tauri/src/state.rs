use std::collections::VecDeque;
use std::sync::Arc;

use serde::Serialize;
use tauri::AppHandle;
use tauri::Manager;
use tokio::sync::Mutex;

/// Server process status.
#[derive(Debug, Clone, Serialize)]
pub struct ServerStatus {
    pub running: bool,
    pub state: String,
    pub uptime_secs: u64,
    pub pid: Option<u32>,
}

impl Default for ServerStatus {
    fn default() -> Self {
        Self {
            running: false,
            state: "stopped".to_string(),
            uptime_secs: 0,
            pid: None,
        }
    }
}

/// Real-time performance metrics.
#[derive(Debug, Clone, Serialize, Default)]
pub struct Metrics {
    pub fps: u32,
    pub bitrate_kbps: u32,
    pub latency_ms: f64,
    pub bytes_sent: u64,
    pub timestamp: u64,
}

/// An active client connection.
#[derive(Debug, Clone, Serialize)]
pub struct Connection {
    pub client_ip: String,
    pub client_name: String,
    pub connected_at: String,
    pub bytes_total: u64,
}

/// A single log entry.
#[derive(Debug, Clone, Serialize)]
pub struct LogEntry {
    pub level: String,
    pub message: String,
    pub timestamp: String,
}

/// macOS permission status.
#[derive(Debug, Clone, Serialize, Default)]
pub struct PermissionStatus {
    pub screen_capture: bool,
    pub accessibility: bool,
    pub microphone: bool,
}

/// Application-wide shared state.
#[derive(Debug)]
pub struct AppState {
    pub status: Mutex<ServerStatus>,
    pub metrics: Mutex<Metrics>,
    pub connections: Mutex<Vec<Connection>>,
    pub logs: Mutex<VecDeque<LogEntry>>,
    pub permissions: Mutex<PermissionStatus>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            status: Mutex::new(ServerStatus::default()),
            metrics: Mutex::new(Metrics::default()),
            connections: Mutex::new(Vec::new()),
            logs: Mutex::new(VecDeque::new()),
            permissions: Mutex::new(PermissionStatus::default()),
        }
    }
}

/// Initialize application state and register it with the Tauri app.
pub fn init_app_state(
    handle: &AppHandle,
) -> Result<(), Box<dyn std::error::Error>> {
    let state = Arc::new(AppState::new());
    handle.manage(state);
    Ok(())
}
