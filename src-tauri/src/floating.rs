use crate::{AppState, desktop};
use deck_core::{
    error::ErrorDto,
    settings::floating::{FloatPreferences, FloatState, validate_alpha},
};
use tauri::{Manager, State};

fn float_only(window: &tauri::WebviewWindow) -> Result<(), ErrorDto> {
    if window.label() != "float" {
        return Err(ErrorDto {
            code: "permission_denied",
            message: "此操作仅用于悬浮窗".into(),
            action: "打开悬浮窗",
        });
    }
    Ok(())
}
#[tauri::command]
pub async fn get_float_state(state: State<'_, AppState>) -> Result<FloatState, ErrorDto> {
    Ok(state.core.float_state().await?)
}
#[tauri::command]
pub async fn save_float_preferences(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    preferences: FloatPreferences,
) -> Result<(), ErrorDto> {
    float_only(&window)?;
    state
        .core
        .save_float_preferences(preferences.clone())
        .await?;
    state
        .float_rounded
        .store(preferences.rounded, std::sync::atomic::Ordering::Release);
    state
        .float_alpha
        .store(preferences.alpha, std::sync::atomic::Ordering::Release);
    apply(&window, preferences).await
}
#[tauri::command]
pub async fn save_float_background(
    window: tauri::WebviewWindow,
    state: State<'_, AppState>,
    background: Option<String>,
) -> Result<(), ErrorDto> {
    float_only(&window)?;
    Ok(state.core.save_float_background(background).await?)
}
#[tauri::command]
pub fn preview_float_alpha(window: tauri::WebviewWindow, alpha: u8) -> Result<(), ErrorDto> {
    float_only(&window)?;
    validate_alpha(alpha)?;
    set_alpha(&window, alpha)?;
    window
        .state::<AppState>()
        .float_alpha
        .store(alpha, std::sync::atomic::Ordering::Release);
    Ok(())
}
pub async fn apply(
    window: &tauri::WebviewWindow,
    preferences: FloatPreferences,
) -> Result<(), ErrorDto> {
    window
        .set_always_on_top(preferences.on_top)
        .map_err(desktop::window_error)?;
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let target = window.clone();
    window
        .run_on_main_thread(move || {
            let result = set_alpha(&target, preferences.alpha)
                .and_then(|_| set_rounded(&target, preferences.rounded));
            let _ = sender.send(result);
        })
        .map_err(desktop::window_error)?;
    receiver.await.map_err(|_| ErrorDto {
        code: "window_failed",
        message: "窗口属性更新被中断".into(),
        action: "重新打开悬浮窗",
    })?
}

#[cfg(target_os = "windows")]
pub fn set_alpha(window: &tauri::WebviewWindow, alpha: u8) -> Result<(), ErrorDto> {
    use windows_sys::Win32::{
        Foundation::{GetLastError, SetLastError},
        UI::WindowsAndMessaging::{
            GWL_EXSTYLE, GetWindowLongPtrW, LWA_ALPHA, SetLayeredWindowAttributes,
            SetWindowLongPtrW, WS_EX_LAYERED,
        },
    };
    let hwnd = window.hwnd().map_err(desktop::window_error)?.0 as _;
    // The HWND is owned by this window; the extended style is preserved except for layered opacity.
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        SetLastError(0);
        if SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style | WS_EX_LAYERED as isize) == 0
            && GetLastError() != 0
        {
            return Err(native_error());
        }
        if SetLayeredWindowAttributes(
            hwnd,
            0,
            (f64::from(alpha) * 255. / 100.).round_ties_even() as u8,
            LWA_ALPHA,
        ) == 0
        {
            return Err(native_error());
        }
    }
    Ok(())
}
#[cfg(target_os = "windows")]
pub fn set_rounded(window: &tauri::WebviewWindow, rounded: bool) -> Result<(), ErrorDto> {
    use windows_sys::Win32::{
        Foundation::RECT,
        Graphics::Gdi::{CreateRoundRectRgn, DeleteObject, SetWindowRgn},
        UI::WindowsAndMessaging::GetClientRect,
    };
    let hwnd = window.hwnd().map_err(desktop::window_error)?.0 as _;
    // Successful SetWindowRgn transfers ownership to Windows. Failed calls must release the region.
    unsafe {
        if !rounded {
            if SetWindowRgn(hwnd, std::ptr::null_mut(), 1) == 0 {
                return Err(native_error());
            }
            return Ok(());
        }
        let mut rect: RECT = std::mem::zeroed();
        if GetClientRect(hwnd, &mut rect) == 0 {
            return Err(native_error());
        }
        let diameter =
            ((14. * window.scale_factor().map_err(desktop::window_error)?) as i32 * 2).max(8);
        let region = CreateRoundRectRgn(0, 0, rect.right + 1, rect.bottom + 1, diameter, diameter);
        if region.is_null() {
            return Err(native_error());
        }
        if SetWindowRgn(hwnd, region, 1) == 0 {
            DeleteObject(region);
            return Err(native_error());
        }
    }
    Ok(())
}
#[cfg(target_os = "windows")]
fn native_error() -> ErrorDto {
    ErrorDto {
        code: "window_failed",
        message: "Windows 未能应用悬浮窗外观".into(),
        action: "重新打开悬浮窗后重试",
    }
}

#[cfg(target_os = "macos")]
pub fn set_alpha(window: &tauri::WebviewWindow, alpha: u8) -> Result<(), ErrorDto> {
    use objc2::{msg_send, runtime::AnyObject};
    let native = window.ns_window().map_err(desktop::window_error)? as *mut AnyObject;
    // Called on the window's UI thread; the pointer remains owned by Tauri.
    unsafe {
        let _: () = msg_send![native, setAlphaValue: (f64::from(alpha) / 100.).max(0.3)];
    }
    Ok(())
}
#[cfg(target_os = "macos")]
pub fn set_rounded(_: &tauri::WebviewWindow, _: bool) -> Result<(), ErrorDto> {
    Ok(())
}
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub fn set_alpha(_: &tauri::WebviewWindow, _: u8) -> Result<(), ErrorDto> {
    Err(ErrorDto {
        code: "unsupported_platform",
        message: "当前平台尚未实现整窗透明度".into(),
        action: "使用 Windows 或 macOS",
    })
}
#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub fn set_rounded(_: &tauri::WebviewWindow, _: bool) -> Result<(), ErrorDto> {
    Err(ErrorDto {
        code: "unsupported_platform",
        message: "当前平台尚未实现原生圆角".into(),
        action: "使用 Windows 或 macOS",
    })
}

pub fn resized(window: &tauri::Window, size: tauri::PhysicalSize<u32>) {
    if window.label() != "float" || size.width == 0 || size.height == 0 {
        return;
    }
    let state = window.state::<AppState>();
    if state.exiting.load(std::sync::atomic::Ordering::Acquire) {
        return;
    }
    let scale = match window.scale_factor() {
        Ok(scale) => scale,
        Err(error) => {
            desktop::report(window.app_handle(), desktop::window_error(error));
            return;
        }
    };
    let logical = size.to_logical::<u32>(scale);
    let core = state.core.clone();
    let app = window.app_handle().clone();
    if let Some(target) = app.get_webview_window("float")
        && let Err(error) = set_rounded(
            &target,
            state
                .float_rounded
                .load(std::sync::atomic::Ordering::Acquire),
        )
    {
        desktop::report(&app, error);
    }
    tauri::async_runtime::spawn(async move {
        if let Err(error) = core.save_float_size(logical.width, logical.height).await {
            desktop::report(&app, error.into());
        }
    });
}
