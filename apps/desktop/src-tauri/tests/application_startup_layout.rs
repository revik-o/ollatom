#![cfg(target_os = "linux")]

#[path = "../src/api/gtk_window_controls.rs"]
mod gtk_window_controls;

use gtk_window_controls::{GtkWindowControls, GtkWindowControlsSide, parse_gtk_window_controls};

#[test]
fn gtk_layout_defaults_to_right_with_standard_order() {
    assert_eq!(
        parse_gtk_window_controls(None),
        GtkWindowControls {
            names: vec!["minimize".into(), "maximize".into(), "close".into()],
            side: GtkWindowControlsSide::Right,
        }
    );
}

#[test]
fn gtk_layout_preserves_declared_control_order() {
    assert_eq!(
        parse_gtk_window_controls(Some("close,maximize,minimize:menu")),
        GtkWindowControls {
            names: vec!["close".into(), "maximize".into(), "minimize".into()],
            side: GtkWindowControlsSide::Left,
        }
    );
}
