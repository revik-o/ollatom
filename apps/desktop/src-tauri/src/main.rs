#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    #[cfg(target_os = "linux")]
    configure_linux_backend();
    app_lib::run();
}

#[cfg(target_os = "linux")]
fn configure_linux_backend() {
    if std::env::var_os("OLLATOM_FORCE_X11").is_some() {
        unsafe { std::env::set_var("GDK_BACKEND", "x11") };
    } else if std::env::var_os("WAYLAND_DISPLAY").is_some()
        && std::env::var_os("GDK_BACKEND").is_none()
    {
        unsafe { std::env::set_var("GDK_BACKEND", "wayland,x11") };
    }
}
