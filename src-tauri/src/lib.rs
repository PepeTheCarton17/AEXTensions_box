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
            commands::auth::get_gh_auth_status,
            commands::auth::login_with_token,
            commands::auth::login_with_cli,
            commands::auth::logout_gh,
            commands::auth::list_github_repos,
            commands::analysis::analyze_url,
            commands::analysis::analyze_multiple_urls,
            commands::generation::generate_extensions,
            commands::catalog::get_created_extensions,
            commands::updates::update_extension,
            commands::updates::update_all_extensions,
            commands::updates::update_extensions_batch,
            commands::catalog::sync_extension_catalog,
            commands::catalog_check::check_extensions_in_repo,
            commands::catalog_check::restore_extension_repo_state,
            commands::deletion::delete_extension,
            commands::deletion::delete_extension_github,
            commands::deletion_permanent::delete_extension_permanently,
            commands::system::get_system_logs,
            commands::system::test_stream_extractor,
            commands::system::get_system_status,
            commands::app_updates::check_aext_updates,
            commands::system::open_url,
            commands::downloads::save_extension_zip,
            commands::files::open_extension_folder,
            commands::files::list_extension_backups,
            commands::files::restore_extension_backup,
            commands::toolchain::check_build_toolchain,
            commands::build::compile_extension,
            commands::actions::refresh_github_actions
        ])
        .run(tauri::generate_context!())
        .expect("error while running AEXT application");
}
