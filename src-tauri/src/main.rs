#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod desktop;

use deck_core::Deck;
use std::{
    path::PathBuf,
    sync::{Arc, atomic::AtomicBool},
};
use tauri::Manager;

struct AppState {
    core: Arc<Deck>,
    exiting: AtomicBool,
    exit_ready: AtomicBool,
}

fn state_root() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 || args[0] != "--state-root" {
        return Err("启动参数：dushan-deck --state-root <绝对数据目录>；开发环境使用 node scripts/deck.mjs dev".into());
    }
    let path = PathBuf::from(&args[1]);
    if !path.is_absolute() {
        return Err("state-root 必须是绝对路径".into());
    }
    Ok(path)
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let root = state_root()?;
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            if let Err(error) = desktop::show(app, "main") {
                desktop::report(app, error);
            }
        }))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .setup(move |app| {
            // Single-instance check precedes setup, so duplicate launches never recover a live DB.
            let core = Arc::new(Deck::open(&root)?);
            let snapshot = tauri::async_runtime::block_on(core.snapshot())?;
            app.manage(AppState {
                core,
                exiting: AtomicBool::new(false),
                exit_ready: AtomicBool::new(false),
            });
            desktop::setup(app, &root, snapshot.settings.float_enabled)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_snapshot,
            commands::unlock_vault,
            commands::lock_vault,
            commands::import_account,
            commands::create_connection,
            commands::save_settings,
            commands::check_storage,
            commands::show_main,
            commands::show_float,
            commands::request_exit
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                if let Err(error) = window.hide() {
                    desktop::report(window.app_handle(), desktop::window_error(error));
                }
            }
        })
        .build(tauri::generate_context!())?;
    app.run(|app, event| {
        if let tauri::RunEvent::ExitRequested { api, .. } = event
            && !app
                .state::<AppState>()
                .exit_ready
                .load(std::sync::atomic::Ordering::Acquire)
        {
            api.prevent_exit();
            desktop::exit(app);
        }
    });
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("Dushan Deck: {error}");
        std::process::exit(1);
    }
}
