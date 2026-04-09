use tauri::AppHandle;

use crate::{
    core::{
        anomaly::{self, AnomalyAlert, AnomalyThresholds},
        diagnosis::{self, DiagnosisError, DiagnosisSummary},
    },
    db,
};

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
    let db = db::get_db_pool(&app_handle).await?;
    let summary = diagnosis::generate_diagnosis_summary(&db, time_range_minutes).await?;
    anomaly::detect_anomalies(&db, &summary, &AnomalyThresholds::default())
        .await
        .map_err(Into::into)
}
