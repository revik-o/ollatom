#![cfg(target_os = "macos")]

use std::ffi::{c_char, c_void};
use tauri::WebviewWindow;

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Point {
    x: f64,
    y: f64,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Size {
    width: f64,
    height: f64,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Rect {
    origin: Point,
    size: Size,
}

#[link(name = "objc")]
unsafe extern "C" {
    fn sel_registerName(name: *const c_char) -> *const c_void;
    fn objc_msgSend();
    #[cfg(target_arch = "x86_64")]
    fn objc_msgSend_stret();
}

pub(crate) fn native_metrics(window: &WebviewWindow) -> Result<(f64, f64), String> {
    let ns_window = window.ns_window().map_err(|error| error.to_string())?;
    if ns_window.is_null() {
        return Err("NSWindow is unavailable".to_owned());
    }
    let selector = unsafe { sel_registerName(b"standardWindowButton:\0".as_ptr().cast()) };
    let send_button: unsafe extern "C" fn(*mut c_void, *const c_void, isize) -> *mut c_void =
        unsafe { std::mem::transmute(objc_msgSend as *const ()) };
    let mut max_x: f64 = 0.0;
    let mut max_height: f64 = 0.0;
    for kind in [0_isize, 1, 2] {
        let button = unsafe { send_button(ns_window.cast(), selector, kind) };
        if button.is_null() {
            continue;
        }
        let frame = object_rect(button, b"frame\0");
        max_x = max_x.max(frame.origin.x + frame.size.width);
        max_height = max_height.max(frame.size.height);
    }
    let window_frame = object_rect(ns_window.cast(), b"frame\0");
    let content_layout = object_rect(ns_window.cast(), b"contentLayoutRect\0");
    let title_bar_height = window_frame.size.height - content_layout.size.height;
    if max_x <= 0.0 || max_height <= 0.0 || title_bar_height <= 0.0 {
        return Err("native traffic-light bounds are unavailable".to_owned());
    }
    Ok((max_x, title_bar_height))
}

fn object_rect(object: *mut c_void, selector_name: &'static [u8]) -> Rect {
    let selector = unsafe { sel_registerName(selector_name.as_ptr().cast()) };
    #[cfg(target_arch = "x86_64")]
    unsafe {
        let mut result = Rect::default();
        let send: unsafe extern "C" fn(*mut Rect, *mut c_void, *const c_void) =
            std::mem::transmute(objc_msgSend_stret as *const ());
        send(&mut result, object, selector);
        result
    }
    #[cfg(not(target_arch = "x86_64"))]
    unsafe {
        let send: unsafe extern "C" fn(*mut c_void, *const c_void) -> Rect =
            std::mem::transmute(objc_msgSend as *const ());
        send(object, selector)
    }
}
