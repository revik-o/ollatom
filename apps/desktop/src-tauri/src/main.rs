#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    #[cfg(target_os = "linux")]
    configure_linux_backend();
    app_lib::run();
}

#[cfg(target_os = "linux")]
fn is_linux_x11_backend() -> bool {
    std::env::var_os("OLLATOM_FORCE_X11").is_some()
}

#[cfg(target_os = "linux")]
fn is_linux_wayland_backend() -> bool {
    std::env::var_os("WAYLAND_DISPLAY").is_some() && std::env::var_os("GDK_BACKEND").is_none()
}

#[cfg(target_os = "linux")]
fn configure_linux_backend() {
    if is_linux_x11_backend() {
        gtk::glib::setenv("GDK_BACKEND", "x11", true)
            .expect("GDK_BACKEND should be configurable before application startup");
    } else if is_linux_wayland_backend() {
        gtk::glib::setenv("GDK_BACKEND", "wayland,x11", false)
            .expect("GDK_BACKEND should be configurable before application startup");
    }
}
