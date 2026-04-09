use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use crate::{
    core::{
        anomaly::{self, AnomalyAlert, AnomalyThresholds},
        diagnosis::{self, DiagnosisError, DiagnosisSummary},
    },
    db,
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
