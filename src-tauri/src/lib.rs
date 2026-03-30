mod cmd;
mod core;
mod db;
mod tray;

use std::sync::Mutex;

use tauri::Manager;

use cmd::MihomoState;
use core::sidecar::SidecarState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(db::plugin::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(
            tauri_plugin_sql::Builder::new()
                .add_migrations(db::migration::DATABASE_URL, db::migration::get_migrations())
                .build(),
        )
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_process::init())
        .manage(SidecarState {
            child: Mutex::new(None),
            log_task: Mutex::new(None),
            traffic_task: Mutex::new(None),
        })
        .manage(MihomoState {
            client: tokio::sync::Mutex::new(core::mihomo::MihomoClient::new(
                "http://127.0.0.1:9090",
                "",
            )),
        })
        .invoke_handler(tauri::generate_handler![
            cmd::sidecar::start_mihomo,
            cmd::sidecar::stop_mihomo,
            cmd::sidecar::restart_mihomo,
            cmd::sidecar::get_mihomo_status,
            cmd::sidecar::check_config_exists,
            cmd::sidecar::ensure_default_config,
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
            cmd::system::update_mihomo_client,
        ])
        .setup(|app| {
            tracing_subscriber::fmt::init();
            tray::create_tray(app.handle())?;
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                let sidecar_state = app.state::<SidecarState>();
                match core::sidecar::stop(&sidecar_state) {
                    Ok(()) => tracing::info!("mihomo sidecar 已在退出时停止"),
                    Err(e) => tracing::warn!("退出时停止 mihomo 失败: {e}"),
                }
            }
        });
}
