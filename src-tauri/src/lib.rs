mod cmd;
mod core;
mod tray;

use std::sync::Mutex;

use tauri::Manager;

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
            client: tokio::sync::Mutex::new(core::mihomo::MihomoClient::new("http://127.0.0.1:9090", "")),
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
            cmd::system::update_mihomo_client,
        ])
        .setup(|app| {
            tracing_subscriber::fmt::init();
            tray::create_tray(app.handle())?;

            // Auto-start mihomo sidecar with default config dir
            let sidecar_state = app.state::<SidecarState>();
            let config_dir = dirs::home_dir()
                .map(|h| h.join(".config").join("mihomo"))
                .unwrap_or_else(|| std::path::PathBuf::from(".config/mihomo"));
            if !config_dir.exists() {
                std::fs::create_dir_all(&config_dir).ok();
            }
            // Ensure config.yaml has external-controller so the API is reachable
            let config_file = config_dir.join("config.yaml");
            let needs_default = if config_file.exists() {
                let content = std::fs::read_to_string(&config_file).unwrap_or_default();
                !content.contains("external-controller")
            } else {
                true
            };
            if needs_default {
                let default_config = "\
mixed-port: 7890\n\
external-controller: 127.0.0.1:9090\n\
";
                std::fs::write(&config_file, default_config).ok();
                tracing::info!("已写入默认 mihomo 配置: {}", config_file.display());
            }
            let config_path = config_dir.to_string_lossy().to_string();
            match core::sidecar::start(app.handle(), &sidecar_state, &config_path) {
                Ok(()) => tracing::info!("mihomo sidecar 已自动启动, config_dir={config_path}"),
                Err(e) => tracing::warn!("mihomo sidecar 自动启动失败: {e}"),
            }

            core::logs::start_log_subscription(app.handle().clone());
            core::traffic::start_traffic_subscription(app.handle().clone());
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
