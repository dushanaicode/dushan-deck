use crate::AppState;
use deck_core::error::ErrorDto;
use std::{path::Path, sync::atomic::Ordering};
use tauri::{
    Emitter, Manager,
    image::Image,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

pub fn window_error(_: tauri::Error) -> ErrorDto {
    ErrorDto {
        code: "window_failed",
        message: "桌面窗口操作失败".into(),
        action: "从托盘重新打开主窗口",
    }
}
pub fn report(app: &tauri::AppHandle, error: ErrorDto) {
    eprintln!("{}: {}", error.code, error.message);
    if let Err(event_error) = app.emit("deck:error", &error) {
        eprintln!("desktop notification failed: {event_error}");
    }
}
pub fn show(app: &tauri::AppHandle, label: &str) -> Result<(), ErrorDto> {
    let window = app.get_webview_window(label).ok_or_else(|| ErrorDto {
        code: "window_starting",
        message: "桌面窗口正在初始化".into(),
        action: "等待窗口启动完成",
    })?;
    window.show().map_err(window_error)?;
    window.unminimize().map_err(window_error)?;
    window.set_focus().map_err(window_error)?;
    Ok(())
}
pub fn exit(app: &tauri::AppHandle) {
    if app.state::<AppState>().exiting.swap(true, Ordering::AcqRel) {
        return;
    }
    if let Err(error) = app.emit("deck:exiting", ()) {
        report(app, window_error(error));
    }
    let app = app.clone();
    let core = app.state::<AppState>().core.clone();
    tauri::async_runtime::spawn(async move {
        let code = match core.shutdown().await {
            Ok(()) => 0,
            Err(error) => {
                report(&app, error.into());
                1
            }
        };
        if let Err(error) = app.global_shortcut().unregister_all() {
            eprintln!("shortcut release failed: {error}");
        }
        app.state::<AppState>()
            .exit_ready
            .store(true, Ordering::Release);
        app.exit(code);
    });
}
fn icon() -> Image<'static> {
    let mut rgba = Vec::with_capacity(32 * 32 * 4);
    for y in 0..32 {
        for x in 0..32 {
            let ink = (7..12).contains(&x) && (7..25).contains(&y)
                || (12..22).contains(&x) && ((7..12).contains(&y) || (20..25).contains(&y))
                || (20..25).contains(&x) && (11..21).contains(&y);
            rgba.extend_from_slice(if ink {
                &[242, 242, 229, 255]
            } else {
                &[35, 67, 53, 255]
            });
        }
    }
    Image::new_owned(rgba, 32, 32)
}
pub fn setup(
    app: &tauri::App,
    root: &Path,
    show_float: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    for (label, title, width, height, visible) in [
        ("main", "Dushan Deck", 1240., 820., true),
        ("float", "Dushan Deck · 悬浮窗", 360., 430., show_float),
    ] {
        let builder = tauri::WebviewWindowBuilder::new(
            app,
            label,
            tauri::WebviewUrl::App(if label == "float" {
                "index.html?surface=float".into()
            } else {
                "index.html".into()
            }),
        )
        .title(title)
        .inner_size(width, height)
        .min_inner_size(if label == "main" { 680. } else { 320. }, 380.)
        .visible(visible)
        .always_on_top(label == "float")
        .skip_taskbar(label == "float")
        .data_directory(root.join("webview"))
        .general_autofill_enabled(false)
        .icon(icon())?
        .on_navigation(|url| {
            matches!(url.scheme(), "tauri")
                || matches!(
                    (url.scheme(), url.host_str()),
                    ("http", Some("tauri.localhost"))
                )
                || (cfg!(debug_assertions)
                    && url.origin().ascii_serialization() == "http://127.0.0.1:1420")
        });
        // Debug-only test access; release builds never open a debugging port.
        #[cfg(debug_assertions)]
        let builder = match std::env::var("DECK_DEVTOOLS_PORT") {
            Ok(port) => {
                let port: u16 = port.parse()?;
                builder.additional_browser_args(&format!("--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection --remote-debugging-port={port} --remote-debugging-address=127.0.0.1"))
            }
            Err(std::env::VarError::NotPresent) => builder,
            Err(error) => return Err(error.into()),
        };
        builder.build()?;
    }
    let main = MenuItem::with_id(app, "main", "打开 Dushan Deck", true, None::<&str>)?;
    let float = MenuItem::with_id(app, "float", "显示悬浮窗", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出 Dushan Deck", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&main, &float, &quit])?;
    TrayIconBuilder::with_id("deck-tray")
        .icon(icon())
        .tooltip("Dushan Deck")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "main" | "float" => {
                if let Err(error) = show(app, event.id.as_ref()) {
                    report(app, error);
                }
            }
            "quit" => exit(app),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                }
            ) && let Err(error) = show(tray.app_handle(), "main")
            {
                report(tray.app_handle(), error);
            }
        })
        .build(app)?;
    app.global_shortcut()
        .on_shortcut("CommandOrControl+Shift+D", |app, _, event| {
            if event.state == ShortcutState::Pressed
                && let Err(error) = show(app, "main")
            {
                report(app, error);
            }
        })?;
    Ok(())
}
