#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum GtkWindowControlsSide {
    Left,
    Right,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GtkWindowControls {
    pub(crate) names: Vec<String>,
    pub(crate) side: GtkWindowControlsSide,
}

pub(crate) fn parse_gtk_window_controls(layout: Option<&str>) -> GtkWindowControls {
    let Some((left_side, right_side)) = layout.unwrap_or_default().split_once(':') else {
        return default_gtk_window_controls();
    };
    let names = left_side
        .split(',')
        .chain(right_side.split(','))
        .map(str::trim)
        .filter(|name| is_window_control_name(name))
        .map(str::to_owned)
        .collect::<Vec<_>>();

    if names.is_empty() {
        return default_gtk_window_controls();
    }

    let controls_are_on_left = has_window_controls(left_side);
    let side = if controls_are_on_left {
        GtkWindowControlsSide::Left
    } else {
        GtkWindowControlsSide::Right
    };

    GtkWindowControls { names, side }
}

fn default_gtk_window_controls() -> GtkWindowControls {
    GtkWindowControls {
        names: vec!["minimize".into(), "maximize".into(), "close".into()],
        side: GtkWindowControlsSide::Right,
    }
}

fn is_window_control_name(name: &str) -> bool {
    matches!(name, "minimize" | "maximize" | "close")
}

fn has_window_controls(layout_side: &str) -> bool {
    layout_side
        .split(',')
        .map(str::trim)
        .any(is_window_control_name)
}
