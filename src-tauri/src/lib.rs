pub mod api;
pub mod commands;
pub mod engine;
pub mod github;
pub mod models;
pub mod state;

use crate::state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let state = AppState::new();

    // Arrancamos el servidor de API local & MCP en segundo plano
    let server_state = state.clone();
    tauri::async_runtime::spawn(async move {
        crate::api::start_embedded_api_server(server_state, 45731).await;
    });

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::get_gh_auth_status,
            commands::login_with_token,
            commands::login_with_cli,
            commands::logout_gh,
            commands::list_github_repos,
            commands::analyze_url,
            commands::analyze_multiple_urls,
            commands::generate_extensions,
            commands::get_created_extensions,
            commands::update_extension,
            commands::delete_extension,
            commands::delete_extension_github,
            commands::get_system_logs,
            commands::test_stream_extractor,
            commands::get_system_status,
            commands::check_aext_updates,
            commands::open_url,
            commands::save_extension_zip,
            commands::open_extension_folder,
            commands::list_extension_backups,
            commands::restore_extension_backup,
            commands::check_build_toolchain,
            commands::compile_extension,
            commands::refresh_github_actions
        ])
        .run(tauri::generate_context!())
        .expect("error while running AEXT application");
}
