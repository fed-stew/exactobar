//! Application windows.

#![allow(dead_code)]

pub mod settings;

use gpui::*;
use tracing::info;

use settings::SettingsWindow;

/// Opens the settings window.
pub fn open_settings(cx: &mut App) {
    info!("Opening settings window");
    
    // CRITICAL: For menu bar apps, we must activate the app first!
    // Without this, open_window() silently fails on macOS.
    cx.activate(true);  // true = ignore other apps
    
    let bounds = Bounds::centered(
        None,
        size(px(700.0), px(500.0)),
        cx,
    );

    let options = WindowOptions {
        titlebar: Some(TitlebarOptions {
            title: Some("ExactoBar Settings".into()),
            appears_transparent: false,
            traffic_light_position: None,
        }),
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        focus: true,
        show: true,
        kind: WindowKind::Normal,
        is_movable: true,
        display_id: None,
        window_background: WindowBackgroundAppearance::Opaque,
        app_id: None,
        window_min_size: Some(size(px(500.0), px(400.0))),
        window_decorations: None,
        is_minimizable: true,
        is_resizable: true,
        tabbing_identifier: None,
    };

    let result = cx.open_window(options, |window, cx| {
        // Activate the window to bring it to front
        window.activate_window();
        cx.new(|_| SettingsWindow::new())
    });
    
    match result {
        Ok(handle) => {
            info!("Settings window opened successfully");
            // Activate the window again via handle to ensure it's in front
            let any_handle: AnyWindowHandle = handle.into();
            let _ = cx.update_window(any_handle, |_, window, _| {
                window.activate_window();
            });
        }
        Err(e) => {
            tracing::error!(error = ?e, "Failed to open settings window");
        }
    }
}

