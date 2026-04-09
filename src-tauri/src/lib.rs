mod cmd;
mod collector;
mod core;
mod db;
mod tray;
mod utils;

use std::{
    future::Future,
    sync::{Arc, Mutex},
    time::Duration,
};

use tauri::Manager;
use tokio::time::{self, Instant as TokioInstant, MissedTickBehavior};

use cmd::MihomoState;
use collector::{CollectorError, CollectorShutdown, CollectorState, RealtimeStore};
use core::{
    anomaly::AlertSeverity,
    notification::{self, NotificationManagerState},
    sidecar::{AiSidecarState, SidecarState},
};
use utils::geoip::{
    default_mihomo_config_dir, resolve_country_mmdb_path, GeoIpConfigState, GeoIpLookup,
};

fn block_on<F: Future>(future: F) -> F::Output {
    if tokio::runtime::Handle::try_current().is_ok() {
        tokio::task::block_in_place(|| tokio::runtime::Handle::current().block_on(future))
    } else {
        tauri::async_runtime::block_on(future)
    }
}

fn start_anomaly_scan_task(app: tauri::AppHandle) {
    tokio::spawn(async move {
        let notification_state = {
            let state = app.state::<NotificationManagerState>();
            Arc::clone(state.inner())
        };
        let mut settings_rx = {
            let manager = notification_state.lock().await;
            manager.subscribe()
        };
        let mut current_settings = settings_rx.borrow().clone();
        let mut interval = build_anomaly_scan_interval(current_settings.scan_interval_secs);

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    if !current_settings.enabled {
                        continue;
                    }

                    let alerts = match cmd::diagnosis::generate_anomaly_alerts(
                        &app,
                        cmd::diagnosis::DEFAULT_DIAGNOSIS_WINDOW_MINUTES,
                    )
                    .await
                    {
                        Ok(alerts) => alerts,
                        Err(error) => {
                            tracing::warn!("定时异常扫描失败: {error}");
                            continue;
                        }
                    };

                    if alerts.is_empty() {
                        continue;
                    }

                    let notifiable_alerts = {
                        let manager = notification_state.lock().await;
                        notification::filter_notifiable_alerts(&manager, &alerts)
                    };

                    if notifiable_alerts.is_empty() {
                        continue;
                    }

                    for alert in notifiable_alerts {
                        let title = match alert.severity {
                            AlertSeverity::Critical => "严重告警",
                            AlertSeverity::Warning => "异常警告",
                            AlertSeverity::Info => "异常提示",
                        };

                        if let Err(error) =
                            notification::send_desktop_notification(&app, title, &alert.title)
                        {
                            tracing::warn!("发送桌面通知失败: {error}");
                            continue;
                        }

                        let mut manager = notification_state.lock().await;
                        manager.mark_notified(notification::alert_type_key(&alert.alert_type));
                    }
                }
                changed = settings_rx.changed() => {
                    if changed.is_err() {
                        break;
                    }

                    current_settings = settings_rx.borrow().clone();
                    interval = build_anomaly_scan_interval(current_settings.scan_interval_secs);
                }
            }
        }
    });
}

fn build_anomaly_scan_interval(scan_interval_secs: u64) -> time::Interval {
    let mut interval = time::interval_at(
        TokioInstant::now() + Duration::from_secs(scan_interval_secs),
        Duration::from_secs(scan_interval_secs),
    );
    interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
    interval
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
        .manage(AiSidecarState::new())
        .manage(GeoIpConfigState::new(default_mihomo_config_dir()))
        .manage(GeoIpLookup::new(resolve_country_mmdb_path(
            &default_mihomo_config_dir(),
        )))
        .manage(MihomoState {
            client: tokio::sync::Mutex::new(core::mihomo::MihomoClient::new(
                "http://127.0.0.1:9090",
                "",
            )),
        })
        .manage(CollectorState::default())
        .manage(RealtimeStore::default())
        .manage(notification::create_notification_manager_state())
        .invoke_handler(tauri::generate_handler![
            cmd::ai::start_ai_service,
            cmd::ai::stop_ai_service,
            cmd::ai::get_ai_status,
            cmd::ai::get_ai_settings,
            cmd::ai::set_ai_settings,
            cmd::ai::ai_ping,
            cmd::ai::test_ai_connection,
            cmd::ai::fetch_ai_models,
            cmd::ai::ai_chat,
            cmd::ai::ai_generate_report,
            cmd::ai::ai_generate_diagnosis,
            cmd::ai::apply_config_change,
            cmd::ai::reject_config_change,
            cmd::ai::list_snapshots,
            cmd::ai::create_snapshot,
            cmd::ai::restore_snapshot,
            cmd::ai::save_conversation_message,
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
            cmd::diagnosis::get_diagnosis_summary,
            cmd::diagnosis::detect_anomalies,
            cmd::diagnosis::get_diagnosis_overview,
            cmd::diagnosis::get_node_health,
            cmd::diagnosis::record_delay_test,
            cmd::diagnosis::get_notification_settings,
            cmd::diagnosis::update_notification_settings,
            cmd::diagnosis::trigger_anomaly_scan,
            cmd::stats::get_domain_stats,
            cmd::stats::get_traffic_hourly,
            cmd::stats::get_traffic_daily,
            cmd::stats::get_stats_overview,
            cmd::stats::get_rule_stats,
            cmd::stats::get_geo_stats,
            cmd::stats::manual_cleanup,
            cmd::stats::get_db_stats,
        ])
        .setup(|app| {
            tracing_subscriber::fmt::init();
            tray::create_tray(app.handle())?;
            collector::start_aggregation_task(app.handle().clone());
            collector::start_cleanup_task(app.handle().clone());

            let app_handle = app.handle().clone();
            let notification_settings =
                block_on(notification::load_notification_settings(&app_handle));
            let notification_state = {
                let state = app_handle.state::<NotificationManagerState>();
                Arc::clone(state.inner())
            };

            block_on(async {
                let mut manager = notification_state.lock().await;
                manager.update_settings(notification_settings);
            });

            start_anomaly_scan_task(app.handle().clone());
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

                let ai_sidecar_state = app.state::<AiSidecarState>();
                match core::sidecar::stop_ai(Some(app), &ai_sidecar_state) {
                    Ok(()) => tracing::info!("ai-service sidecar 已在退出时停止"),
                    Err(core::sidecar::AiSidecarError::NotRunning) => {}
                    Err(e) => tracing::warn!("退出时停止 ai-service 失败: {e}"),
                }
            }
        });
}
