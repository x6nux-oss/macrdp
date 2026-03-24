use tauri::tray::{MouseButton, MouseButtonState, TrayIconEvent};
use tauri::{AppHandle, LogicalPosition, Manager};

/// Set up the system tray icon event handling.
///
/// The tray icon itself is created from `tauri.conf.json` (id: "main-tray").
/// This function retrieves it and registers the click handler to toggle
/// the popover window.
pub fn setup_tray(handle: &AppHandle) -> Result<(), Box<dyn std::error::Error>> {
    let tray = handle
        .tray_by_id("main-tray")
        .ok_or("tray icon 'main-tray' not found")?;

    tray.set_tooltip(Some("macrdp"))?;

    let app_handle = handle.clone();
    tray.on_tray_icon_event(move |_tray, event| {
        if let TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        } = event
        {
            toggle_popover(&app_handle);
        }
    });

    Ok(())
}

/// Toggle the popover window visibility.
///
/// If the popover is currently visible, hide it.
/// If hidden, position it near the top-right of the screen (below the menu bar),
/// then show and focus it.
fn toggle_popover(handle: &AppHandle) {
    if let Some(window) = handle.get_webview_window("popover") {
        match window.is_visible() {
            Ok(true) => {
                let _ = window.hide();
            }
            _ => {
                position_popover_near_tray(handle, &window);
                let _ = window.show();
                let _ = window.set_focus();
            }
        }
    }
}

/// Position the popover window near the tray icon area (top-right of screen).
///
/// Places the window at `(screen_width - 320, 28)` in logical pixels,
/// where 28 accounts for the macOS menu bar height.
fn position_popover_near_tray(
    handle: &AppHandle,
    window: &tauri::WebviewWindow,
) {
    const MENU_BAR_HEIGHT: f64 = 28.0;
    const POPOVER_WIDTH: f64 = 320.0;

    if let Ok(Some(monitor)) = handle.primary_monitor() {
        let size = monitor.size();
        let scale = monitor.scale_factor();
        let logical_width = size.width as f64 / scale;
        let x = logical_width - POPOVER_WIDTH;
        let _ = window.set_position(LogicalPosition::new(x, MENU_BAR_HEIGHT));
    }
}

/// Update the tray icon title to reflect the current server status.
///
/// On macOS the tray title is displayed as text next to the icon:
/// - `"connected"` — show bitrate and latency
/// - `"running"` / `"stopped"` / default — clear the title
pub fn update_tray_status(
    handle: &AppHandle,
    state: &str,
    bitrate_kbps: Option<u32>,
    latency_ms: Option<f64>,
) {
    let Some(tray) = handle.tray_by_id("main-tray") else {
        return;
    };

    match state {
        "connected" => {
            let mbps = bitrate_kbps.unwrap_or(0) as f64 / 1000.0;
            let ms = latency_ms.unwrap_or(0.0);
            let title = format!("{:.1} Mbps | {:.0} ms", mbps, ms);
            let _ = tray.set_title(Some(&title));
        }
        _ => {
            let _ = tray.set_title(None::<&str>);
        }
    }
}
