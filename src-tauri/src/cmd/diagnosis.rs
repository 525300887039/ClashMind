use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::{
    core::{
        anomaly::{self, AnomalyAlert, AnomalyThresholds},
        diagnosis::{self, DiagnosisError, DiagnosisSummary},
        node_health::{self, NodeHealthScore},
        notification::{self, NotificationError, NotificationManagerState, NotificationSettings},
    },
    db::{self, repo_node_health, repo_node_health::NodeHealthInsert, DbError},
};

pub(crate) const DEFAULT_DIAGNOSIS_WINDOW_MINUTES: i32 = 30;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosisOverview {
    pub summary: DiagnosisSummary,
    pub alerts: Vec<AnomalyAlert>,
}

async fn generate_diagnosis_overview(
    app_handle: &AppHandle,
    time_range_minutes: i32,
) -> Result<DiagnosisOverview, DiagnosisError> {
    let db = db::get_db_pool(app_handle).await?;
    let summary = diagnosis::generate_diagnosis_summary(&db, time_range_minutes).await?;
    let alerts = anomaly::detect_anomalies(&db, &summary, &AnomalyThresholds::default()).await?;

    Ok(DiagnosisOverview { summary, alerts })
}

pub(crate) async fn generate_anomaly_alerts(
    app_handle: &AppHandle,
    time_range_minutes: i32,
) -> Result<Vec<AnomalyAlert>, DiagnosisError> {
    let db = db::get_db_pool(app_handle).await?;
    let summary = diagnosis::generate_diagnosis_summary(&db, time_range_minutes).await?;
    anomaly::detect_anomalies(&db, &summary, &AnomalyThresholds::default())
        .await
        .map_err(DiagnosisError::from)
}

async fn generate_node_health_scores(
    app_handle: &AppHandle,
    hours: i32,
) -> Result<Vec<NodeHealthScore>, DbError> {
    let db = db::get_db_pool(app_handle).await?;
    let aggregates = repo_node_health::get_all_nodes_health(&db, hours).await?;
    Ok(node_health::calculate_all_health_scores(&aggregates))
}

#[tauri::command]
pub async fn get_diagnosis_summary(
    app_handle: AppHandle,
    time_range_minutes: Option<i32>,
) -> Result<DiagnosisSummary, DiagnosisError> {
    let time_range_minutes = time_range_minutes.unwrap_or(DEFAULT_DIAGNOSIS_WINDOW_MINUTES);
    let db = db::get_db_pool(&app_handle).await?;
    diagnosis::generate_diagnosis_summary(&db, time_range_minutes).await
}

#[tauri::command]
pub async fn detect_anomalies(
    app_handle: AppHandle,
    time_range_minutes: Option<i32>,
) -> Result<Vec<AnomalyAlert>, DiagnosisError> {
    let time_range_minutes = time_range_minutes.unwrap_or(DEFAULT_DIAGNOSIS_WINDOW_MINUTES);
    generate_anomaly_alerts(&app_handle, time_range_minutes).await
}

#[tauri::command]
pub async fn get_diagnosis_overview(
    app_handle: AppHandle,
    time_range_minutes: Option<i32>,
) -> Result<DiagnosisOverview, DiagnosisError> {
    let time_range_minutes = time_range_minutes.unwrap_or(DEFAULT_DIAGNOSIS_WINDOW_MINUTES);
    generate_diagnosis_overview(&app_handle, time_range_minutes).await
}

#[tauri::command]
pub async fn get_node_health(
    app_handle: AppHandle,
    hours: Option<i32>,
) -> Result<Vec<NodeHealthScore>, DbError> {
    generate_node_health_scores(&app_handle, hours.unwrap_or(24)).await
}

#[tauri::command]
pub async fn record_delay_test(
    app_handle: AppHandle,
    node_name: String,
    delay_ms: Option<i32>,
    success: bool,
) -> Result<(), DbError> {
    let db = db::get_db_pool(&app_handle).await?;
    repo_node_health::insert_health_snapshot(
        &db,
        &NodeHealthInsert {
            node_name,
            delay_ms,
            success,
        },
    )
    .await?;

    Ok(())
}

#[tauri::command]
pub async fn get_notification_settings(
    app_handle: AppHandle,
) -> Result<NotificationSettings, NotificationError> {
    let state = app_handle.state::<NotificationManagerState>();
    let manager = state.inner().lock().await;

    Ok(manager.settings())
}

#[tauri::command]
pub async fn update_notification_settings(
    app_handle: AppHandle,
    settings: NotificationSettings,
) -> Result<(), NotificationError> {
    let settings = settings.normalized();
    notification::store_notification_settings(&app_handle, &settings).await?;

    let state = app_handle.state::<NotificationManagerState>();
    let mut manager = state.inner().lock().await;
    manager.update_settings(settings);

    Ok(())
}

#[tauri::command]
pub async fn trigger_anomaly_scan(
    app_handle: AppHandle,
) -> Result<Vec<AnomalyAlert>, DiagnosisError> {
    generate_anomaly_alerts(&app_handle, DEFAULT_DIAGNOSIS_WINDOW_MINUTES).await
}
