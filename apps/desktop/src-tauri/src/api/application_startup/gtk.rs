use super::*;

#[cfg(target_os = "linux")]
pub(super) fn gtk_window_controls(window: &WebviewWindow) -> GtkWindowControls {
    use ::gtk::prelude::{GtkSettingsExt, WidgetExt};
    let layout = window
        .gtk_window()
        .ok()
        .and_then(|window| window.settings())
        .and_then(|settings| settings.gtk_decoration_layout());

    parse_gtk_window_controls(layout.as_deref())
}

#[cfg(target_os = "linux")]
pub(super) fn window_controls_side(side: GtkWindowControlsSide) -> WindowControlsSide {
    (side == GtkWindowControlsSide::Left)
        .then_some(WindowControlsSide::Left)
        .unwrap_or(WindowControlsSide::Right)
}

#[cfg(target_os = "linux")]
pub(super) fn install_gtk_layout_listener(window: &WebviewWindow, handle: AppHandle) {
    use ::gtk::prelude::{GtkSettingsExt, WidgetExt};

    let gtk_settings = window.gtk_window().ok().and_then(|window| window.settings());

    let Some(settings) = gtk_settings else {
        return;
    };

    settings.connect_gtk_decoration_layout_notify(move |settings| {
        let layout = settings.gtk_decoration_layout();
        let gtk_window_controls = parse_gtk_window_controls(layout.as_deref());
        let controls_side = window_controls_side(gtk_window_controls.side);
        let state = handle.state::<StartupState>();
        let updated_snapshot = state.update_linux_controls(controls_side, gtk_window_controls.names);

        if let Ok(snapshot) = updated_snapshot {
            let _ = handle.emit(WINDOW_APPEARANCE_CHANGED_EVENT, snapshot);
        }
    });
}
