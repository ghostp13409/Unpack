pub mod ai;
pub mod apply;
pub mod commands;
pub mod db;
pub mod headless;
pub mod ingest;
pub mod mcp;
pub mod models;
pub mod ranking;
pub mod repo;
pub mod secrets;
pub mod settings;

use commands::AppState;
use std::sync::Mutex;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let context = tauri::generate_context!();
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("--mcp") {
        std::process::exit(mcp::serve_stdio(&context.config().identifier));
    }
    if let Some(code) = headless::maybe_run(&context.config().identifier, &args) {
        std::process::exit(code);
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        // No args: the registered launch command must never look like --mcp or a headless run.
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            let data_dir = app.path().app_data_dir()?;
            let conn = db::open(&data_dir.join("tasks.db"))?;
            // Per-capture PDF copies live here; anything left from a crash goes now.
            let _ = std::fs::remove_dir_all(data_dir.join("tmp"));
            let status = Mutex::new(settings::load_status(&conn));
            app.manage(AppState { db: Mutex::new(conn), data_dir, status });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::list_tasks,
            commands::get_task_detail,
            commands::create_task,
            commands::update_task,
            commands::add_note,
            commands::delete_note,
            commands::list_categories,
            commands::up_next,
            commands::start_capture,
            commands::apply_capture,
            commands::discard_capture,
            commands::list_captures,
            commands::undo_change,
            commands::get_settings,
            commands::save_settings,
            commands::set_api_key,
            commands::clear_api_key,
            commands::test_connection,
            commands::probe_provider,
            commands::open_data_folder,
        ])
        .run(context)
        .expect("error while building tauri application");
}
