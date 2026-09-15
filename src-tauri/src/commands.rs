use crate::{AppState, desktop};
use deck_core::{
    Snapshot,
    accounts::{ImportAccount, ImportResult},
    catalog::CreateConnection,
    error::ErrorDto,
    settings::Settings,
};
use tauri::{Emitter, Manager, State};

pub(crate) fn main_only(window: &tauri::WebviewWindow) -> Result<(), ErrorDto> {
    if window.label() != "main" {
        return Err(ErrorDto {
            code: "permission_denied",
            message: "请在主窗口进行此操作".into(),
            action: "打开主窗口",
        });
    }
    Ok(())
}
fn changed(app: &tauri::AppHandle) {
    // A lost notification does not turn an already-committed mutation into a reported failure.
    if let Err(error) = app.emit("deck:changed", ()) {
        desktop::report(app, desktop::window_error(error));
    }
}
#[tauri::command]
pub async fn get_snapshot(state: State<'_, AppState>) -> Result<Snapshot, ErrorDto> {
    Ok(state.core.snapshot().await?)
}
#[tauri::command]
pub async fn unlock_vault(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    password: String,
) -> Result<(), ErrorDto> {
    main_only(&window)?;
    state.core.unlock(password).await?;
    changed(&app);
    crate::usage::refresh_in_background(&app);
    Ok(())
}
#[tauri::command]
pub async fn lock_vault(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<(), ErrorDto> {
    main_only(&window)?;
    state.core.lock().await?;
    changed(&app);
    Ok(())
}
#[tauri::command]
pub async fn import_account(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    request: ImportAccount,
) -> Result<ImportResult, ErrorDto> {
    main_only(&window)?;
    let result = state.core.import(request).await?;
    changed(&app);
    crate::usage::refresh_in_background(&app);
    Ok(result)
}
#[tauri::command]
pub async fn create_connection(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    request: CreateConnection,
) -> Result<String, ErrorDto> {
    main_only(&window)?;
    let id = state.core.create_connection(request).await?;
    changed(&app);
    Ok(id)
}
#[tauri::command]
pub async fn save_settings(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    settings: Settings,
) -> Result<(), ErrorDto> {
    main_only(&window)?;
    let float_enabled = settings.float_enabled;
    state.core.save_settings(settings).await?;
    changed(&app);
    if float_enabled {
        desktop::show(&app, "float")?;
    } else {
        app.get_webview_window("float")
            .expect("float window exists")
            .hide()
            .map_err(desktop::window_error)?;
    }
    Ok(())
}
#[tauri::command]
pub async fn check_storage(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
    state: State<'_, AppState>,
) -> Result<String, ErrorDto> {
    main_only(&window)?;
    let id = state.core.check_storage().await?;
    changed(&app);
    Ok(id)
}
#[tauri::command]
pub fn show_main(app: tauri::AppHandle) -> Result<(), ErrorDto> {
    desktop::show(&app, "main")
}
#[tauri::command]
pub fn show_float(app: tauri::AppHandle) -> Result<(), ErrorDto> {
    desktop::show(&app, "float")
}
#[tauri::command]
pub fn request_exit(app: tauri::AppHandle, window: tauri::WebviewWindow) -> Result<(), ErrorDto> {
    main_only(&window)?;
    desktop::exit(&app);
    Ok(())
}
