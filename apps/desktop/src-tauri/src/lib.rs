mod api;

pub use api::application_configuration::validate_application_configuration_value;

#[cfg(desktop)]
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let application_builder = tauri::Builder::default();

    #[cfg(desktop)]
    let application_builder = application_builder.plugin(tauri_plugin_single_instance::init(
        |application, _arguments, _current_working_directory| {
            if let Some(main_window) = application.get_webview_window("main") {
                let _show_result = main_window.show();
                let _unminimize_result = main_window.unminimize();
                let _focus_result = main_window.set_focus();
            }
        },
    ));

    application_builder
        .setup(|app| {
            api::application_configuration::initialize(app)?;

            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            api::application_configuration::get_application_config_value_by_key,
            api::application_configuration::set_application_config_value
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
