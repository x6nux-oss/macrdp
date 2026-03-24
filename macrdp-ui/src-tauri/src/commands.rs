use std::sync::Arc;

use tauri::Emitter;
use tauri::Manager;
use tauri::State;

use crate::cli_manager::CliManager;
use crate::database::Database;
use crate::ipc_client::IpcClient;
use crate::state::AppState;

#[tauri::command]
pub async fn start_server(
    app: tauri::AppHandle,
    cli: State<'_, Arc<CliManager>>,
    ipc: State<'_, Arc<IpcClient>>,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    let socket_path = cli.socket_path().clone();
    tracing::info!("start_server called, socket_path={:?}", socket_path);

    // Check if already running via our managed child process
    if cli.is_running().await {
        tracing::info!("CLI already running, checking IPC connection");
        // Already running, ensure IPC is connected
        if ipc.send_command("").await.is_err() {
            // IPC not connected, try to connect
            let _ = ipc.connect(&socket_path, app.clone()).await;
        }
        return Ok(());
    }

    // Check if the socket file exists (could be stale or another instance)
    if socket_path.exists() {
        match tokio::net::UnixStream::connect(&socket_path).await {
            Ok(_stream) => {
                // Socket is live — another CLI instance is already running.
                // Just connect our IPC client to it.
                ipc.connect(&socket_path, app.clone()).await?;

                // Update status to reflect running state
                {
                    let mut status = state.status.lock().await;
                    status.running = true;
                    status.state = "running".to_string();
                }
                let _ = app.emit("server-status", &*state.status.lock().await);
                return Ok(());
            }
            Err(_) => {
                // Stale socket file — remove it before spawning
                let _ = std::fs::remove_file(&socket_path);
            }
        }
    }

    // Spawn the CLI process
    tracing::info!("Spawning CLI process...");
    cli.start().await.map_err(|e| {
        tracing::error!("CLI start failed: {}", e);
        e
    })?;
    tracing::info!("CLI process spawned, waiting for socket...");

    // Wait briefly for the server to create its IPC socket
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Connect IPC client
    let ipc_ref = Arc::clone(&*ipc);
    let socket = socket_path.clone();
    let app_handle = app.clone();

    // Spawn a background task that connects with retry
    tokio::spawn(async move {
        for attempt in 0..10 {
            match ipc_ref.connect(&socket, app_handle.clone()).await {
                Ok(()) => {
                    tracing::info!("IPC connected on attempt {}", attempt + 1);
                    return;
                }
                Err(e) => {
                    tracing::debug!(
                        "IPC connect attempt {} failed: {}",
                        attempt + 1,
                        e
                    );
                    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
                }
            }
        }
        tracing::error!("Failed to connect IPC after 10 attempts");
    });

    // Update status to starting
    {
        let mut status = state.status.lock().await;
        status.running = true;
        status.state = "starting".to_string();
        status.pid = cli.pid().await;
    }
    let _ = app.emit("server-status", &*state.status.lock().await);

    Ok(())
}

#[tauri::command]
pub async fn stop_server(
    app: tauri::AppHandle,
    cli: State<'_, Arc<CliManager>>,
    ipc: State<'_, Arc<IpcClient>>,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    // Try graceful shutdown via IPC first
    let cmd_id = uuid::Uuid::new_v4().to_string();
    let shutdown_cmd =
        serde_json::json!({"id": cmd_id, "cmd": "shutdown"}).to_string();

    let graceful = ipc.send_command(&shutdown_cmd).await.is_ok();

    if graceful {
        // Wait up to 2 seconds for process to exit
        for _ in 0..20 {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            if !cli.is_running().await {
                break;
            }
        }
    }

    // If still running, force kill
    if cli.is_running().await {
        cli.stop().await?;
    }

    // Update status
    {
        let mut status = state.status.lock().await;
        status.running = false;
        status.state = "stopped".to_string();
        status.uptime_secs = 0;
        status.pid = None;
    }
    let _ = app.emit("server-status", &*state.status.lock().await);

    // Clean up socket file
    let socket_path = cli.socket_path();
    if socket_path.exists() {
        let _ = std::fs::remove_file(socket_path);
    }

    Ok(())
}

#[tauri::command]
pub async fn get_server_status(
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, String> {
    let status = state.status.lock().await.clone();
    serde_json::to_value(&status).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_metrics(
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, String> {
    let metrics = state.metrics.lock().await.clone();
    serde_json::to_value(&metrics).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_connections(
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, String> {
    let connections = state.connections.lock().await.clone();
    serde_json::to_value(&connections).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_permissions(
    state: State<'_, Arc<AppState>>,
) -> Result<serde_json::Value, String> {
    let perms = state.permissions.lock().await.clone();
    serde_json::to_value(&perms).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_config(
    ipc: State<'_, Arc<IpcClient>>,
) -> Result<serde_json::Value, String> {
    let cmd_id = uuid::Uuid::new_v4().to_string();
    let cmd =
        serde_json::json!({"id": cmd_id, "cmd": "get_config"}).to_string();
    ipc.send_command(&cmd).await?;
    // The response arrives asynchronously via the IPC read loop
    // and is emitted as an "ipc-response" Tauri event.
    Ok(serde_json::json!({"status": "requested", "id": cmd_id}))
}

#[tauri::command]
pub async fn set_config(
    config: serde_json::Value,
    ipc: State<'_, Arc<IpcClient>>,
) -> Result<(), String> {
    let cmd_id = uuid::Uuid::new_v4().to_string();
    let cmd = serde_json::json!({
        "id": cmd_id,
        "cmd": "set_config",
        "config": config,
    })
    .to_string();
    ipc.send_command(&cmd).await
}

#[tauri::command]
pub async fn get_logs(
    limit: Option<usize>,
    state: State<'_, Arc<AppState>>,
    db: State<'_, Arc<Database>>,
) -> Result<Vec<serde_json::Value>, String> {
    let limit = limit.unwrap_or(100);

    // Try in-memory logs first
    let logs = state.logs.lock().await;
    if !logs.is_empty() {
        let entries: Vec<serde_json::Value> = logs
            .iter()
            .rev()
            .take(limit)
            .map(|e| serde_json::to_value(e).unwrap_or_default())
            .collect();
        return Ok(entries);
    }

    // Fall back to database
    db.get_logs(limit)
}

#[tauri::command]
pub fn get_connection_history(
    limit: Option<u32>,
    offset: Option<u32>,
    db: State<'_, Arc<Database>>,
) -> Result<Vec<serde_json::Value>, String> {
    db.get_connection_history(limit.unwrap_or(50), offset.unwrap_or(0))
}

#[tauri::command]
pub fn get_traffic_stats(
    days: Option<u32>,
    db: State<'_, Arc<Database>>,
) -> Result<Vec<serde_json::Value>, String> {
    db.get_traffic_stats(days.unwrap_or(30))
}

#[tauri::command]
pub fn check_for_updates() -> Result<serde_json::Value, String> {
    Ok(serde_json::json!({
        "current_version": env!("CARGO_PKG_VERSION"),
        "available": false,
    }))
}

#[tauri::command]
pub fn show_main_window(app: tauri::AppHandle) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("main") {
        window.show().map_err(|e: tauri::Error| e.to_string())?;
        window
            .set_focus()
            .map_err(|e: tauri::Error| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub fn open_system_preferences(pane: String) -> Result<(), String> {
    crate::permissions::open_system_preferences(&pane)
}

#[tauri::command]
pub async fn quit_app(
    app: tauri::AppHandle,
    cli: State<'_, Arc<CliManager>>,
    ipc: State<'_, Arc<IpcClient>>,
) -> Result<(), String> {
    // If CLI is running, try graceful shutdown first
    if cli.is_running().await {
        let cmd_id = uuid::Uuid::new_v4().to_string();
        let shutdown_cmd =
            serde_json::json!({"id": cmd_id, "cmd": "shutdown"}).to_string();
        let _ = ipc.send_command(&shutdown_cmd).await;

        // Wait up to 2 seconds for graceful shutdown
        for _ in 0..20 {
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            if !cli.is_running().await {
                break;
            }
        }

        // Force kill if still running
        if cli.is_running().await {
            let _ = cli.stop().await;
        }
    }

    app.exit(0);
    Ok(())
}

#[tauri::command]
pub fn set_autostart(
    enabled: bool,
    app: tauri::AppHandle,
) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    let autolaunch = app.autolaunch();
    if enabled {
        autolaunch
            .enable()
            .map_err(|e| format!("Failed to enable autostart: {}", e))?;
    } else {
        autolaunch
            .disable()
            .map_err(|e| format!("Failed to disable autostart: {}", e))?;
    }
    Ok(())
}
