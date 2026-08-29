#![cfg(windows)]

use super::application_startup::LogicalRect;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use tauri::WebviewWindow;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Dwm::{
    DWMWA_CAPTION_BUTTON_BOUNDS, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_ROUND, DwmDefWindowProc,
    DwmGetWindowAttribute, DwmSetWindowAttribute,
};
use windows::Win32::UI::Controls::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::WindowsAndMessaging::{
    GetClientRect, GetSystemMetricsForDpi, GetWindowRect, HTBOTTOM, HTBOTTOMLEFT, HTBOTTOMRIGHT,
    HTCAPTION, HTCLIENT, HTLEFT, HTRIGHT, HTTOP, HTTOPLEFT, HTTOPRIGHT, SM_CXPADDEDBORDER,
    SM_CXSIZEFRAME, SM_CYSIZEFRAME, ScreenToClient, WM_DPICHANGED, WM_DWMCOMPOSITIONCHANGED,
    WM_NCCALCSIZE, WM_NCDESTROY, WM_NCHITTEST, WM_SETTINGCHANGE, WM_SIZE,
};

const SUBCLASS_ID: usize = 0x4f4c_4c41;
#[derive(Clone, Default)]
struct HitTestState {
    interactive_regions: Vec<LogicalRect>,
    scale_factor: f64,
    title_bar_height: f64,
}

static WINDOWS: OnceLock<Mutex<HashMap<isize, HitTestState>>> = OnceLock::new();

fn states() -> &'static Mutex<HashMap<isize, HitTestState>> {
    WINDOWS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn key(hwnd: HWND) -> isize {
    hwnd.0 as isize
}

pub(crate) fn initialize(window: &WebviewWindow) -> Result<(f64, f64), String> {
    let hwnd = window.hwnd().map_err(|error| error.to_string())?;
    let scale_factor = window.scale_factor().unwrap_or(1.0);
    let bounds = caption_button_bounds(hwnd)
        .filter(|bounds| bounds.right > bounds.left && bounds.bottom > bounds.top)
        .ok_or_else(|| "Windows did not expose native caption-button bounds".to_owned())?;
    let width = (bounds.right - bounds.left) as f64 / scale_factor;
    let title_bar_height = (bounds.bottom - bounds.top) as f64 / scale_factor;
    unsafe {
        SetWindowSubclass(hwnd, Some(window_proc), SUBCLASS_ID, 0)
            .map_err(|error| error.to_string())?;
        let preference = DWMWCP_ROUND;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &preference as *const _ as *const _,
            std::mem::size_of_val(&preference) as u32,
        );
    }
    states()
        .lock()
        .map_err(|_| "window hit-test state is unavailable".to_owned())?
        .insert(
            key(hwnd),
            HitTestState {
                interactive_regions: Vec::new(),
                scale_factor,
                title_bar_height,
            },
        );
    Ok((width, title_bar_height))
}

pub(crate) fn set_interactive_regions(
    window: &WebviewWindow,
    values: Vec<LogicalRect>,
) -> Result<(), String> {
    let hwnd = window.hwnd().map_err(|error| error.to_string())?;
    let mut all = states()
        .lock()
        .map_err(|_| "window hit-test state is unavailable".to_owned())?;
    let state = all.entry(key(hwnd)).or_default();
    state.interactive_regions = values;
    state.scale_factor = window.scale_factor().unwrap_or(1.0);
    Ok(())
}

fn caption_button_bounds(hwnd: HWND) -> Option<RECT> {
    let mut bounds = RECT::default();
    unsafe {
        DwmGetWindowAttribute(
            hwnd,
            DWMWA_CAPTION_BUTTON_BOUNDS,
            &mut bounds as *mut _ as *mut _,
            std::mem::size_of::<RECT>() as u32,
        )
        .ok()?;
    }
    Some(bounds)
}

unsafe extern "system" fn window_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _subclass_id: usize,
    _ref_data: usize,
) -> LRESULT {
    let mut dwm_result = LRESULT(0);
    if message == WM_NCHITTEST
        && unsafe { DwmDefWindowProc(hwnd, message, wparam, lparam, &mut dwm_result) }.as_bool()
    {
        return dwm_result;
    }

    match message {
        WM_NCCALCSIZE if wparam.0 != 0 => return LRESULT(0),
        WM_NCHITTEST => return LRESULT(hit_test(hwnd, lparam) as isize),
        WM_DPICHANGED | WM_SIZE | WM_SETTINGCHANGE | WM_DWMCOMPOSITIONCHANGED => {
            refresh_scale(hwnd)
        }
        WM_NCDESTROY => {
            let _ = states().lock().map(|mut all| all.remove(&key(hwnd)));
            let _ = unsafe { RemoveWindowSubclass(hwnd, Some(window_proc), SUBCLASS_ID) };
        }
        _ => {}
    }
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}

fn hit_test(hwnd: HWND, lparam: LPARAM) -> i32 {
    let screen_x = (lparam.0 as i16) as i32;
    let screen_y = ((lparam.0 >> 16) as i16) as i32;
    let mut window_rect = RECT::default();
    let mut client_rect = RECT::default();
    if unsafe { GetWindowRect(hwnd, &mut window_rect) }.is_err()
        || unsafe { GetClientRect(hwnd, &mut client_rect) }.is_err()
    {
        return HTCLIENT.0;
    }
    let dpi = unsafe { GetDpiForWindow(hwnd) }.max(96);
    let frame_x = unsafe { GetSystemMetricsForDpi(SM_CXSIZEFRAME, dpi) }
        + unsafe { GetSystemMetricsForDpi(SM_CXPADDEDBORDER, dpi) };
    let frame_y = unsafe { GetSystemMetricsForDpi(SM_CYSIZEFRAME, dpi) }
        + unsafe { GetSystemMetricsForDpi(SM_CXPADDEDBORDER, dpi) };
    let left = screen_x < window_rect.left + frame_x;
    let right = screen_x >= window_rect.right - frame_x;
    let top = screen_y < window_rect.top + frame_y;
    let bottom = screen_y >= window_rect.bottom - frame_y;
    match (left, right, top, bottom) {
        (true, _, true, _) => return HTTOPLEFT.0,
        (_, true, true, _) => return HTTOPRIGHT.0,
        (true, _, _, true) => return HTBOTTOMLEFT.0,
        (_, true, _, true) => return HTBOTTOMRIGHT.0,
        (_, _, true, _) => return HTTOP.0,
        (_, _, _, true) => return HTBOTTOM.0,
        (true, _, _, _) => return HTLEFT.0,
        (_, true, _, _) => return HTRIGHT.0,
        _ => {}
    }

    let mut client_point = POINT {
        x: screen_x,
        y: screen_y,
    };
    let _ = unsafe { ScreenToClient(hwnd, &mut client_point) };
    let state = states()
        .lock()
        .ok()
        .and_then(|all| all.get(&key(hwnd)).cloned())
        .unwrap_or_default();
    let scale = if state.scale_factor > 0.0 {
        state.scale_factor
    } else {
        dpi as f64 / 96.0
    };
    let logical = (client_point.x as f64 / scale, client_point.y as f64 / scale);
    if state.interactive_regions.iter().any(|rect| {
        logical.0 >= rect.x
            && logical.1 >= rect.y
            && logical.0 <= rect.x + rect.width
            && logical.1 <= rect.y + rect.height
    }) {
        return HTCLIENT.0;
    }
    if logical.1 <= state.title_bar_height {
        HTCAPTION.0
    } else {
        HTCLIENT.0
    }
}

fn refresh_scale(hwnd: HWND) {
    let scale = unsafe { GetDpiForWindow(hwnd) }.max(96) as f64 / 96.0;
    if let Ok(mut all) = states().lock() {
        if let Some(state) = all.get_mut(&key(hwnd)) {
            state.scale_factor = scale;
        }
    }
}
