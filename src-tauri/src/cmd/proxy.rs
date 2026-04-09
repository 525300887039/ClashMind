use std::collections::HashMap;

use tauri::{AppHandle, Manager};

use crate::core::mihomo::MihomoError;
use crate::db::{self, repo_node_health, repo_node_health::NodeHealthInsert};

use super::MihomoState;

fn delay_ms_from_u32(delay_ms: u32) -> Option<i32> {
    i32::try_from(delay_ms).ok()
}

async fn persist_delay_snapshot(app_handle: &AppHandle, snapshot: NodeHealthInsert) {
    let node_name = snapshot.node_name.clone();
    let db = match db::get_db_pool(app_handle).await {
        Ok(db) => db,
        Err(error) => {
            tracing::warn!("记录节点健康快照前获取数据库失败: node={node_name}, error={error}");
            return;
        }
    };

    if let Err(error) = repo_node_health::insert_health_snapshot(&db, &snapshot).await {
        tracing::warn!("记录节点健康快照失败: node={node_name}, error={error}");
    }
}

async fn persist_delay_snapshots(app_handle: &AppHandle, snapshots: Vec<NodeHealthInsert>) {
    if snapshots.is_empty() {
        return;
    }

    let db = match db::get_db_pool(app_handle).await {
        Ok(db) => db,
        Err(error) => {
            tracing::warn!("批量记录节点健康快照前获取数据库失败: error={error}");
            return;
        }
    };

    if let Err(error) = repo_node_health::insert_health_snapshots_batch(&db, &snapshots).await {
        tracing::warn!("批量记录节点健康快照失败: error={error}");
    }
}

pub(crate) async fn run_delay_test_and_record(
    app_handle: &AppHandle,
    name: &str,
    url: &str,
    timeout: u32,
) -> Result<u32, MihomoError> {
    let mihomo_state = app_handle.state::<MihomoState>();
    let result = mihomo_state
        .client
        .lock()
        .await
        .test_delay(name, url, timeout)
        .await;

    match result {
        Ok(delay_ms) => {
            persist_delay_snapshot(
                app_handle,
                NodeHealthInsert {
                    node_name: name.to_string(),
                    delay_ms: delay_ms_from_u32(delay_ms),
                    success: true,
                },
            )
            .await;
            Ok(delay_ms)
        }
        Err(error) => {
            persist_delay_snapshot(
                app_handle,
                NodeHealthInsert {
                    node_name: name.to_string(),
                    delay_ms: None,
                    success: false,
                },
            )
            .await;
            Err(error)
        }
    }
}

pub(crate) async fn run_group_delay_test_and_record(
    app_handle: &AppHandle,
    group: &str,
    url: &str,
    timeout: u32,
) -> Result<HashMap<String, u32>, MihomoError> {
    let mihomo_state = app_handle.state::<MihomoState>();
    let delay_results = mihomo_state
        .client
        .lock()
        .await
        .test_group_delay(group, url, timeout)
        .await?;

    let snapshots = delay_results
        .iter()
        .map(|(node_name, delay_ms)| NodeHealthInsert {
            node_name: node_name.clone(),
            delay_ms: delay_ms_from_u32(*delay_ms),
            success: true,
        })
        .collect::<Vec<_>>();
    persist_delay_snapshots(app_handle, snapshots).await;

    Ok(delay_results)
}

#[tauri::command]
pub async fn get_proxies(
    state: tauri::State<'_, MihomoState>,
) -> Result<serde_json::Value, MihomoError> {
    state.client.lock().await.get_proxies().await
}

#[tauri::command]
pub async fn switch_proxy(
    state: tauri::State<'_, MihomoState>,
    group: String,
    name: String,
) -> Result<(), MihomoError> {
    state.client.lock().await.switch_proxy(&group, &name).await
}

#[tauri::command]
pub async fn test_delay(
    app_handle: AppHandle,
    name: String,
    url: String,
    timeout: u32,
) -> Result<u32, MihomoError> {
    run_delay_test_and_record(&app_handle, &name, &url, timeout).await
}

#[tauri::command]
pub async fn test_group_delay(
    app_handle: AppHandle,
    group: String,
    url: String,
    timeout: u32,
) -> Result<HashMap<String, u32>, MihomoError> {
    run_group_delay_test_and_record(&app_handle, &group, &url, timeout).await
}

#[tauri::command]
pub async fn get_rules(
    state: tauri::State<'_, MihomoState>,
) -> Result<serde_json::Value, MihomoError> {
    state.client.lock().await.get_rules().await
}
