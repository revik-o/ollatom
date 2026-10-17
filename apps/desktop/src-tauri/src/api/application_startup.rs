use crate::api::application_infrastructure;
#[cfg(target_os = "linux")]
use crate::api::gtk_window_controls::{GtkWindowControls, GtkWindowControlsSide, parse_gtk_window_controls};
use infrastructure::JsonConfigurationStore;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State, WebviewWindow};
use tokio::sync::Notify;

#[cfg(target_os = "linux")]
mod gtk;
mod state;
mod window_appearance;
#[cfg(target_os = "linux")]
use gtk::install_gtk_layout_listener;
use window_appearance::{apply_backdrop, initial_chrome_metrics};

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
    pub configuration: JsonConfigurationStore,
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
            let configuration = infrastructure::create_json_configuration_store(
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
pub(crate) async fn wait_for_background_ready(startup: State<'_, StartupState>) -> Result<StartupSnapshot, String> {
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

    Ok(())
}

fn is_valid_interactive_region(region: &LogicalRect) -> bool {
    let has_finite_origin = region.x.is_finite() && region.y.is_finite();
    let has_finite_size = region.width.is_finite() && region.height.is_finite();
    let has_non_negative_size = region.width >= 0.0 && region.height >= 0.0;

    has_finite_origin && has_finite_size && has_non_negative_size
}
