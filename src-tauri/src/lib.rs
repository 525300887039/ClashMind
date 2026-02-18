mod cmd;
mod core;
mod tray;

use std::sync::Mutex;

use cmd::MihomoState;
use core::sidecar::SidecarState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_sql::Builder::new().build())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_process::init())
        .manage(SidecarState {
            child: Mutex::new(None),
        })
        .manage(MihomoState {
            client: core::mihomo::MihomoClient::new("http://127.0.0.1:9090", ""),
        })
        .invoke_handler(tauri::generate_handler![
            cmd::sidecar::start_mihomo,
            cmd::sidecar::stop_mihomo,
            cmd::sidecar::restart_mihomo,
            cmd::sidecar::get_mihomo_status,
            cmd::proxy::get_proxies,
            cmd::proxy::switch_proxy,
            cmd::proxy::test_delay,
            cmd::proxy::test_group_delay,
            cmd::proxy::get_rules,
            cmd::config::get_configs,
            cmd::config::patch_configs,
            cmd::config::read_config,
            cmd::config::write_config,
            cmd::config::reload_config,
            cmd::system::get_version,
            cmd::system::close_connection,
            cmd::system::close_all_connections,
            cmd::system::get_connections,
            cmd::system::set_system_proxy,
            cmd::system::get_system_proxy,
        ])
        .setup(|app| {
            tracing_subscriber::fmt::init();
            tray::create_tray(app.handle())?;
            core::logs::start_log_subscription(app.handle().clone());
            core::traffic::start_traffic_subscription(app.handle().clone());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
