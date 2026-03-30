mod cmd;
mod collector;
mod core;
mod db;
mod tray;

use std::{future::Future, sync::Mutex};

use tauri::Manager;

use cmd::MihomoState;
use collector::{CollectorError, CollectorShutdown, CollectorState, RealtimeStore};
use core::sidecar::SidecarState;

fn block_on<F: Future>(future: F) -> F::Output {
    if tokio::runtime::Handle::try_current().is_ok() {
        tokio::task::block_in_place(|| tokio::runtime::Handle::current().block_on(future))
    } else {
        tauri::async_runtime::block_on(future)
    }
}

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
        .manage(CollectorState::default())
        .manage(RealtimeStore::default())
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
            cmd::collector::start_collector,
            cmd::collector::stop_collector,
            cmd::collector::get_collector_status,
            cmd::collector::get_realtime_connections,
            cmd::collector::get_realtime_summary,
        ])
        .setup(|app| {
            tracing_subscriber::fmt::init();
            tray::create_tray(app.handle())?;
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if let tauri::RunEvent::ExitRequested { .. } = event {
                let collector_result = block_on(async {
                    let collector_state = app.state::<CollectorState>();
                    let _operation_guard = collector_state.lock_operation().await;
                    collector_state.cleanup_finished().await?;

                    match collector_state
                        .stop_runtime(CollectorShutdown::StopAndCloseActive)
                        .await
                    {
                        Ok(()) | Err(CollectorError::NotRunning) => Ok(()),
                        Err(error) => Err(error),
                    }
                });

                if let Err(error) = collector_result {
                    tracing::warn!("退出时等待 collector flush 失败: {error}");
                }

                let sidecar_state = app.state::<SidecarState>();
                match core::sidecar::stop(&sidecar_state) {
                    Ok(()) => tracing::info!("mihomo sidecar 已在退出时停止"),
                    Err(e) => tracing::warn!("退出时停止 mihomo 失败: {e}"),
                }
            }
        });
}
