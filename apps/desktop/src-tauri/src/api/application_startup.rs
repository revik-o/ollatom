use crate::api::application_infrastructure;
use infrastructure::YamlConfigurationStore;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State, WebviewWindow};
use tokio::sync::Notify;

const APPEARANCE_CHANGED_EVENT: &str = "window-appearance-changed";
const APPLICATION_CONFIGURATION_FILE_NAME: &str = "application.yaml";

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)]
pub enum BackdropKind {
    Acrylic,
    Vibrancy,
    WaylandBlur,
    Opaque,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)]
pub enum WindowChromeMode {
    NativeOverlay,
    ClientDrawn,
    NativeStandard,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowControlsSide {
    Left,
    Right,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowChromeMetrics {
    pub mode: WindowChromeMode,
    pub controls: Vec<String>,
    pub title_bar_height: f64,
    pub controls_side: WindowControlsSide,
    pub controls_inset_start: f64,
    pub controls_inset_end: f64,
    pub scale_factor: f64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupSnapshot {
    pub backdrop: BackdropKind,
    pub chrome: WindowChromeMetrics,
}

#[derive(Clone)]
pub struct StartupResources {
    pub configuration: YamlConfigurationStore,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogicalRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

pub struct StartupState {
    resources: Mutex<Option<Result<StartupResources, String>>>,
    resources_ready: Notify,
    snapshot: Mutex<StartupSnapshot>,
    #[allow(dead_code)]
    interactive_regions: Mutex<Vec<LogicalRect>>,
}

impl StartupState {
    fn new(snapshot: StartupSnapshot) -> Self {
        Self {
            resources: Mutex::new(None),
            resources_ready: Notify::new(),
            snapshot: Mutex::new(snapshot),
            interactive_regions: Mutex::new(Vec::new()),
        }
    }

    pub async fn resources(&self) -> Result<StartupResources, String> {
        loop {
            let notified = self.resources_ready.notified();

            if let Some(value) = self
                .resources
                .lock()
                .map_err(|_| "startup state is unavailable")?
                .clone()
            {
                return value;
            }

            notified.await;
        }
    }

    fn complete_resources(&self, value: Result<StartupResources, String>) {
        if let Ok(mut resources) = self.resources.lock()
            && resources.is_none()
        {
            *resources = Some(value);
            self.resources_ready.notify_waiters();
        }
    }

    fn snapshot(&self) -> Result<StartupSnapshot, String> {
        self.snapshot
            .lock()
            .map(|value| value.clone())
            .map_err(|_| "window startup state is unavailable".to_owned())
    }

    fn update_backdrop(&self, backdrop: BackdropKind) -> Result<StartupSnapshot, String> {
        let mut snapshot = self
            .snapshot
            .lock()
            .map_err(|_| "window startup state is unavailable".to_owned())?;
        snapshot.backdrop = backdrop;

        Ok(snapshot.clone())
    }

    #[cfg(target_os = "linux")]
    fn update_linux_controls(
        &self,
        controls_side: WindowControlsSide,
        controls: Vec<String>,
    ) -> Result<StartupSnapshot, String> {
        let mut snapshot = self
            .snapshot
            .lock()
            .map_err(|_| "window startup state is unavailable".to_owned())?;
        snapshot.chrome.controls_side = controls_side;
        snapshot.chrome.controls = controls;

        Ok(snapshot.clone())
    }
}

pub(crate) fn initialize(application: &mut tauri::App) -> Result<(), Box<dyn Error>> {
    let window = application
        .get_webview_window("main")
        .ok_or("the main window is missing from the Tauri configuration")?;

    let theme = if matches!(window.theme(), Ok(tauri::Theme::Dark)) {
        "black"
    } else {
        "white"
    };

    application.manage(StartupState::new(StartupSnapshot {
        backdrop: apply_backdrop(&window, theme),
        chrome: initial_chrome_metrics(&window),
    }));

    #[cfg(target_os = "linux")]
    install_gtk_layout_listener(&window, application.handle().clone());

    window.show()?;

    let handle = application.handle().clone();
    let configuration_directory = application.path().app_config_dir()?;
    let data_directory = application.path().app_local_data_dir()?;

    tauri::async_runtime::spawn(async move {
        let result = async {
            let configuration = infrastructure::create_yaml_configuration_file(
                APPLICATION_CONFIGURATION_FILE_NAME,
                configuration_directory,
            )
            .await
            .map_err(|error| error.to_string())?;
            let infrastructure = application_infrastructure::initialize(&data_directory).await?;

            if !handle.manage(infrastructure) {
                return Err("application infrastructure has already been initialized".to_owned());
            }

            Ok(StartupResources { configuration })
        }
        .await;

        let state = handle.state::<StartupState>();
        state.complete_resources(result);

        if let Ok(snapshot) = state.snapshot() {
            let _ = handle.emit(APPEARANCE_CHANGED_EVENT, snapshot);
        }
    });
    Ok(())
}

#[tauri::command]
pub(crate) async fn wait_for_background_ready(
    startup: State<'_, StartupState>,
) -> Result<StartupSnapshot, String> {
    startup.resources().await?;
    startup.snapshot()
}

#[tauri::command]
pub(crate) fn set_window_appearance(
    app: AppHandle,
    startup: State<'_, StartupState>,
    theme: String,
) -> Result<BackdropKind, String> {
    if theme != "black" && theme != "white" {
        return Err("theme must be 'black' or 'white'".to_owned());
    }

    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "the main window is unavailable".to_owned())?;
    let backdrop = apply_backdrop(&window, &theme);
    let snapshot = startup.update_backdrop(backdrop.clone())?;
    let _ = app.emit(APPEARANCE_CHANGED_EVENT, snapshot);

    Ok(backdrop)
}

#[tauri::command]
pub(crate) fn set_window_interactive_regions(
    _app: AppHandle,
    startup: State<'_, StartupState>,
    regions: Vec<LogicalRect>,
) -> Result<(), String> {
    if regions.iter().any(|r| {
        !r.x.is_finite()
            || !r.y.is_finite()
            || !r.width.is_finite()
            || !r.height.is_finite()
            || r.width < 0.0
            || r.height < 0.0
    }) {
        return Err("interactive regions must be finite, non-negative rectangles".to_owned());
    }

    *startup
        .interactive_regions
        .lock()
        .map_err(|_| "window startup state is unavailable")? = regions;

    #[cfg(windows)]
    if let Some(window) = _app.get_webview_window("main") {
        let stored = startup
            .interactive_regions
            .lock()
            .map_err(|_| "window startup state is unavailable")?
            .clone();
        crate::api::windows_chrome::set_interactive_regions(&window, stored)?;
    }

    Ok(())
}

fn initial_chrome_metrics(window: &WebviewWindow) -> WindowChromeMetrics {
    let scale_factor = window.scale_factor().unwrap_or(1.0);

    #[cfg(target_os = "linux")]
    return WindowChromeMetrics {
        mode: WindowChromeMode::ClientDrawn,
        controls: gtk_controls(window),
        title_bar_height: 40.0,
        controls_side: gtk_controls_side(window),
        controls_inset_start: 0.0,
        controls_inset_end: 0.0,
        scale_factor,
    };

    #[cfg(target_os = "macos")]
    {
        if let Ok((controls_inset_start, title_bar_height)) =
            crate::api::macos_chrome::native_metrics(window)
        {
            return WindowChromeMetrics {
                mode: WindowChromeMode::NativeOverlay,
                controls: Vec::new(),
                title_bar_height,
                controls_side: WindowControlsSide::Left,
                controls_inset_start,
                controls_inset_end: 0.0,
                scale_factor,
            };
        }
        return WindowChromeMetrics {
            mode: WindowChromeMode::NativeStandard,
            controls: Vec::new(),
            title_bar_height: 0.0,
            controls_side: WindowControlsSide::Left,
            controls_inset_start: 0.0,
            controls_inset_end: 0.0,
            scale_factor,
        };
    }

    #[cfg(windows)]
    {
        return match crate::api::windows_chrome::initialize(window) {
            Ok((controls_inset_end, title_bar_height)) => WindowChromeMetrics {
                mode: WindowChromeMode::NativeOverlay,
                controls: Vec::new(),
                title_bar_height,
                controls_side: WindowControlsSide::Right,
                controls_inset_start: 0.0,
                controls_inset_end,
                scale_factor,
            },
            Err(_) => WindowChromeMetrics {
                mode: WindowChromeMode::NativeStandard,
                controls: Vec::new(),
                title_bar_height: 0.0,
                controls_side: WindowControlsSide::Right,
                controls_inset_start: 0.0,
                controls_inset_end: 0.0,
                scale_factor,
            },
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

fn apply_backdrop(window: &WebviewWindow, theme: &str) -> BackdropKind {
    #[cfg(windows)]
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
                    .effect(Effect::Acrylic)
                    .color(tint)
                    .build(),
            )
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

#[cfg(target_os = "linux")]
fn gtk_controls_side(window: &WebviewWindow) -> WindowControlsSide {
    use gtk::prelude::{GtkSettingsExt, WidgetExt};
    let layout = window
        .gtk_window()
        .ok()
        .and_then(|window| window.settings())
        .and_then(|settings| settings.gtk_decoration_layout());
    parse_gtk_decoration_layout(layout.as_deref())
}

#[cfg(target_os = "linux")]
fn gtk_controls(window: &WebviewWindow) -> Vec<String> {
    use gtk::prelude::{GtkSettingsExt, WidgetExt};
    let layout = window
        .gtk_window()
        .ok()
        .and_then(|window| window.settings())
        .and_then(|settings| settings.gtk_decoration_layout());
    parse_gtk_controls(layout.as_deref())
}

#[cfg(target_os = "linux")]
fn parse_gtk_controls(layout: Option<&str>) -> Vec<String> {
    let controls = layout
        .unwrap_or_default()
        .split_once(':')
        .into_iter()
        .flat_map(|(left, right)| left.split(',').chain(right.split(',')))
        .map(str::trim)
        .filter(|item| matches!(*item, "minimize" | "maximize" | "close"))
        .map(str::to_owned)
        .collect::<Vec<_>>();

    if controls.is_empty() {
        vec!["minimize".into(), "maximize".into(), "close".into()]
    } else {
        controls
    }
}

#[cfg(target_os = "linux")]
fn parse_gtk_decoration_layout(layout: Option<&str>) -> WindowControlsSide {
    let Some((left, _right)) = layout.unwrap_or_default().split_once(':') else {
        return WindowControlsSide::Right;
    };
    let has_controls = |side: &str| {
        side.split(',')
            .any(|item| matches!(item.trim(), "minimize" | "maximize" | "close"))
    };

    if has_controls(left) {
        WindowControlsSide::Left
    } else {
        WindowControlsSide::Right
    }
}

#[cfg(target_os = "linux")]
fn install_gtk_layout_listener(window: &WebviewWindow, handle: AppHandle) {
    use gtk::prelude::{GtkSettingsExt, WidgetExt};

    let Some(settings) = window
        .gtk_window()
        .ok()
        .and_then(|window| window.settings())
    else {
        return;
    };

    settings.connect_gtk_decoration_layout_notify(move |settings| {
        let layout = settings.gtk_decoration_layout();
        let side = parse_gtk_decoration_layout(layout.as_deref());
        let controls = parse_gtk_controls(layout.as_deref());
        let state = handle.state::<StartupState>();
        if let Ok(snapshot) = state.update_linux_controls(side, controls) {
            let _ = handle.emit(APPEARANCE_CHANGED_EVENT, snapshot);
        }
    });
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::{WindowControlsSide, parse_gtk_controls, parse_gtk_decoration_layout};

    #[test]
    fn gtk_layout_defaults_to_right_with_standard_order() {
        assert!(matches!(
            parse_gtk_decoration_layout(None),
            WindowControlsSide::Right
        ));
        assert_eq!(
            parse_gtk_controls(None),
            vec!["minimize", "maximize", "close"]
        );
    }

    #[test]
    fn gtk_layout_preserves_declared_control_order() {
        assert!(matches!(
            parse_gtk_decoration_layout(Some("close,maximize,minimize:menu")),
            WindowControlsSide::Left
        ));
        assert_eq!(
            parse_gtk_controls(Some("close,maximize,minimize:menu")),
            vec!["close", "maximize", "minimize"]
        );
    }
}
