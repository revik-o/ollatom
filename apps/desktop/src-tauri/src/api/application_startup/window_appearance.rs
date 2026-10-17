#[cfg(target_os = "linux")]
use super::gtk::{gtk_window_controls, window_controls_side};
use super::*;

pub(super) fn initial_chrome_metrics(window: &WebviewWindow) -> WindowChromeMetrics {
    let scale_factor = window.scale_factor().unwrap_or(1.0);

    #[cfg(target_os = "linux")]
    {
        let gtk_window_controls = gtk_window_controls(window);
        return WindowChromeMetrics {
            mode: WindowChromeMode::ClientDrawn,
            controls: gtk_window_controls.names,
            title_bar_height: 40.0,
            controls_side: window_controls_side(gtk_window_controls.side),
            controls_inset_start: 0.0,
            controls_inset_end: 0.0,
            scale_factor,
        };
    }

    #[cfg(any(target_os = "macos", windows))]
    {
        return WindowChromeMetrics {
            mode: WindowChromeMode::NativeStandard,
            controls: Vec::new(),
            title_bar_height: 0.0,
            #[cfg(target_os = "macos")]
            controls_side: WindowControlsSide::Left,
            #[cfg(windows)]
            controls_side: WindowControlsSide::Right,
            controls_inset_start: 0.0,
            controls_inset_end: 0.0,
            scale_factor,
        };
    }

    #[allow(unreachable_code)]
    WindowChromeMetrics {
        mode: WindowChromeMode::NativeStandard,
        controls: Vec::new(),
        title_bar_height: 0.0,
        controls_side: WindowControlsSide::Right,
        controls_inset_start: 0.0,
        controls_inset_end: 0.0,
        scale_factor,
    }
}

pub(super) fn apply_backdrop(window: &WebviewWindow, theme: &str) -> BackdropKind {
    #[cfg(windows)]
    {
        use tauri::window::{Color, Effect, EffectsBuilder};

        let tint = if theme == "black" {
            Color(0, 0, 0, 218)
        } else {
            Color(255, 255, 255, 202)
        };

        return window
            .set_effects(EffectsBuilder::new().effect(Effect::Acrylic).color(tint).build())
            .map(|_| BackdropKind::Acrylic)
            .unwrap_or(BackdropKind::Opaque);
    }
    #[cfg(all(target_os = "macos", feature = "macos-direct"))]
    {
        use tauri::window::{Color, Effect, EffectsBuilder};

        let tint = if theme == "black" {
            Color(0, 0, 0, 218)
        } else {
            Color(255, 255, 255, 202)
        };

        return window
            .set_effects(
                EffectsBuilder::new()
                    .effect(Effect::UnderWindowBackground)
                    .color(tint)
                    .build(),
            )
            .map(|_| BackdropKind::Vibrancy)
            .unwrap_or(BackdropKind::Opaque);
    }
    #[cfg(all(target_os = "linux", feature = "wayland-blur"))]
    {
        let _ = theme;
        return window_vibrancy_wayland::apply_blur(window, None)
            .map(|_| BackdropKind::WaylandBlur)
            .unwrap_or(BackdropKind::Opaque);
    }

    #[cfg(not(any(
        windows,
        all(target_os = "macos", feature = "macos-direct"),
        all(target_os = "linux", feature = "wayland-blur")
    )))]
    {
        let _ = (window, theme);
        BackdropKind::Opaque
    }
}
