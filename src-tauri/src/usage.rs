use crate::{AppState, desktop};
use deck_core::{
    Deck,
    error::ErrorDto,
    usage::types::{QuotaSnapshot, UsageSource},
};
use std::sync::{Arc, atomic::Ordering};
use tauri::{Emitter, Manager, State};

#[tauri::command]
pub async fn get_quota_snapshot(state: State<'_, AppState>) -> Result<QuotaSnapshot, ErrorDto> {
    Ok(state.core.quota_snapshot().await?)
}
#[tauri::command]
pub async fn refresh_quotas(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    force: bool,
) -> Result<QuotaSnapshot, ErrorDto> {
    let snapshot = state.core.refresh_quotas(force).await?;
    changed(&app);
    Ok(snapshot)
}
#[tauri::command]
pub async fn get_usage_sources(state: State<'_, AppState>) -> Result<Vec<UsageSource>, ErrorDto> {
    Ok(state.core.usage_sources().await?)
}
#[tauri::command]
pub async fn save_usage_source(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    source: UsageSource,
) -> Result<String, ErrorDto> {
    crate::commands::main_only(&window)?;
    Ok(state.core.save_usage_source(source).await?)
}
#[tauri::command]
pub async fn remove_usage_source(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    id: String,
) -> Result<(), ErrorDto> {
    crate::commands::main_only(&window)?;
    Ok(state.core.remove_usage_source(id).await?)
}
fn changed(app: &tauri::AppHandle) {
    if app.state::<AppState>().exiting.load(Ordering::Acquire) {
        return;
    }
    if let Err(error) = app.emit("deck:quota", ()) {
        desktop::report(app, desktop::window_error(error));
    }
}
pub fn refresh_in_background(app: &tauri::AppHandle) {
    let app = app.clone();
    let core = app.state::<AppState>().core.clone();
    tauri::async_runtime::spawn(async move {
        match core.refresh_quotas(false).await {
            Ok(_) => changed(&app),
            Err(error) => {
                if !app.state::<AppState>().exiting.load(Ordering::Acquire) {
                    desktop::report(&app, error.into());
                }
            }
        }
    });
}
pub fn schedule(
    app: tauri::AppHandle,
    core: Arc<Deck>,
    mut stopped: tokio::sync::watch::Receiver<bool>,
) {
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(60));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {
                _ = stopped.changed() => break,
                _ = interval.tick() => {
                    tokio::select! {
                        _ = stopped.changed() => break,
                        result = core.refresh_quotas(false) => match result { Ok(_) => changed(&app), Err(error) => { if !app.state::<AppState>().exiting.load(Ordering::Acquire) { desktop::report(&app, error.into()); } } }
                    }
                }
            }
        }
    });
}
