use tauri::Manager;
use tokio::sync::watch;

use crate::collector::{
    ws_client::{self, ConnectionRecord},
    CollectorError, CollectorShutdown, CollectorState, RealtimeStore, RealtimeSummary,
};

use super::MihomoState;

#[tauri::command]
pub async fn start_collector(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, CollectorState>,
    mihomo_state: tauri::State<'_, MihomoState>,
) -> Result<(), CollectorError> {
    let _operation_guard = state.lock_operation().await;
    state.cleanup_finished().await?;
    state.begin_start()?;

    let (api_address, api_secret) = {
        let client = mihomo_state.client.lock().await;
        client.connection_info()
    };

    let (cancel_tx, cancel_rx) = watch::channel(CollectorShutdown::Run);
    let (done_tx, done_rx) = watch::channel(false);
    app_handle.state::<RealtimeStore>().reset().await;
    let handle = app_handle.clone();
    let task = tauri::async_runtime::spawn(async move {
        ws_client::run_connections_collector(
            handle,
            api_address,
            api_secret,
            cancel_rx,
            done_tx,
        )
        .await;
    });

    if let Err(error) = state.start_runtime(cancel_tx, done_rx, task) {
        let _ = state.abort_start();
        return Err(error);
    }

    tracing::info!("collector 启动成功");
    Ok(())
}

#[tauri::command]
pub async fn stop_collector(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, CollectorState>,
) -> Result<(), CollectorError> {
    let _operation_guard = state.lock_operation().await;
    state.cleanup_finished().await?;
    state.stop_runtime(CollectorShutdown::Stop).await?;
    app_handle.state::<RealtimeStore>().reset().await;
    tracing::info!("collector 已停止");
    Ok(())
}

#[tauri::command]
pub async fn get_collector_status(
    state: tauri::State<'_, CollectorState>,
) -> Result<bool, CollectorError> {
    let _operation_guard = state.lock_operation().await;
    state.cleanup_finished().await?;
    Ok(state.is_running())
}

#[tauri::command]
pub async fn get_realtime_connections(
    store: tauri::State<'_, RealtimeStore>,
) -> Result<Vec<ConnectionRecord>, CollectorError> {
    Ok(store.get_active_connections().await)
}

#[tauri::command]
pub async fn get_realtime_summary(
    store: tauri::State<'_, RealtimeStore>,
) -> Result<RealtimeSummary, CollectorError> {
    Ok(store.get_summary().await)
}
