use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::{
    core::{
        anomaly::{self, AnomalyAlert, AnomalyThresholds},
        diagnosis::{self, DiagnosisError, DiagnosisSummary},
        node_health::{self, NodeHealthScore},
    },
    db::{self, repo_node_health, repo_node_health::NodeHealthInsert, DbError},
};

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
    let time_range_minutes = time_range_minutes.unwrap_or(30);
    let db = db::get_db_pool(&app_handle).await?;
    diagnosis::generate_diagnosis_summary(&db, time_range_minutes).await
}

#[tauri::command]
pub async fn detect_anomalies(
    app_handle: AppHandle,
    time_range_minutes: Option<i32>,
) -> Result<Vec<AnomalyAlert>, DiagnosisError> {
    let time_range_minutes = time_range_minutes.unwrap_or(30);
    let overview = generate_diagnosis_overview(&app_handle, time_range_minutes).await?;
    Ok(overview.alerts)
}

#[tauri::command]
pub async fn get_diagnosis_overview(
    app_handle: AppHandle,
    time_range_minutes: Option<i32>,
) -> Result<DiagnosisOverview, DiagnosisError> {
    let time_range_minutes = time_range_minutes.unwrap_or(30);
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
