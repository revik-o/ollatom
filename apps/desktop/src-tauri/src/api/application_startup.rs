use crate::api::application_infrastructure;
#[cfg(target_os = "linux")]
use crate::api::gtk_window_controls::{
    GtkWindowControls, GtkWindowControlsSide, parse_gtk_window_controls,
};
use infrastructure::YamlConfigurationStore;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State, WebviewWindow};
use tokio::sync::Notify;

const WINDOW_APPEARANCE_CHANGED_EVENT: &str = "window-appearance-changed";
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
            let resources = self
                .resources
                .lock()
                .map_err(|_| "startup state is unavailable")?
                .clone();

            if let Some(value) = resources {
                return value;
            }

            notified.await;
        }
    }

    fn complete_resources(&self, value: Result<StartupResources, String>) {
        let Ok(mut resources) = self.resources.lock() else {
            return;
        };
        if resources.is_some() {
            return;
        }

        *resources = Some(value);
        self.resources_ready.notify_waiters();
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
            let _ = handle.emit(WINDOW_APPEARANCE_CHANGED_EVENT, snapshot);
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
    let _ = app.emit(WINDOW_APPEARANCE_CHANGED_EVENT, snapshot);

    Ok(backdrop)
}

#[tauri::command]
pub(crate) fn set_window_interactive_regions(
    _app: AppHandle,
    startup: State<'_, StartupState>,
    regions: Vec<LogicalRect>,
) -> Result<(), String> {
    let are_regions_valid = regions.iter().all(is_valid_interactive_region);
    if !are_regions_valid {
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

fn is_valid_interactive_region(region: &LogicalRect) -> bool {
    let has_finite_origin = region.x.is_finite() && region.y.is_finite();
    let has_finite_size = region.width.is_finite() && region.height.is_finite();
    let has_non_negative_size = region.width >= 0.0 && region.height >= 0.0;

    has_finite_origin && has_finite_size && has_non_negative_size
}

fn initial_chrome_metrics(window: &WebviewWindow) -> WindowChromeMetrics {
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

    #[cfg(target_os = "macos")]
    {
        let native_metrics = crate::api::macos_chrome::native_metrics(window);
        if let Ok((controls_inset_start, title_bar_height)) = native_metrics {
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
fn gtk_window_controls(window: &WebviewWindow) -> GtkWindowControls {
    use gtk::prelude::{GtkSettingsExt, WidgetExt};
    let layout = window
        .gtk_window()
        .ok()
        .and_then(|window| window.settings())
        .and_then(|settings| settings.gtk_decoration_layout());
    parse_gtk_window_controls(layout.as_deref())
}

#[cfg(target_os = "linux")]
fn window_controls_side(side: GtkWindowControlsSide) -> WindowControlsSide {
    if side == GtkWindowControlsSide::Left {
        WindowControlsSide::Left
    } else {
        WindowControlsSide::Right
    }
}

#[cfg(target_os = "linux")]
fn install_gtk_layout_listener(window: &WebviewWindow, handle: AppHandle) {
    use gtk::prelude::{GtkSettingsExt, WidgetExt};

    let gtk_settings = window
        .gtk_window()
        .ok()
        .and_then(|window| window.settings());
    let Some(settings) = gtk_settings else {
        return;
    };

    settings.connect_gtk_decoration_layout_notify(move |settings| {
        let layout = settings.gtk_decoration_layout();
        let gtk_window_controls = parse_gtk_window_controls(layout.as_deref());
        let controls_side = window_controls_side(gtk_window_controls.side);
        let state = handle.state::<StartupState>();
        let updated_snapshot =
            state.update_linux_controls(controls_side, gtk_window_controls.names);
        if let Ok(snapshot) = updated_snapshot {
            let _ = handle.emit(WINDOW_APPEARANCE_CHANGED_EVENT, snapshot);
        }
    });
}
