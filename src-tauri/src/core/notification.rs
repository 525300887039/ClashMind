//! Desktop notification management for anomaly alerts.

use std::{
    collections::{HashMap, HashSet},
    io::ErrorKind,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tauri_plugin_notification::{NotificationExt, PermissionState};
use thiserror::Error;
use tokio::sync::{watch, Mutex};

use crate::core::anomaly::{AlertSeverity, AlertType, AnomalyAlert};

const MIN_SCAN_INTERVAL_SECS: u64 = 60;
const NOTIFICATION_SETTINGS_FILE_NAME: &str = "notification-settings.json";

/// Shared application state for notification management.
pub type NotificationManagerState = Arc<Mutex<NotificationManager>>;

/// Notification settings surfaced to the frontend.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NotificationSettings {
    pub enabled: bool,
    pub critical_only: bool,
    pub scan_interval_secs: u64,
}

impl Default for NotificationSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            critical_only: false,
            scan_interval_secs: 300,
        }
    }
}

#[derive(Debug)]
pub struct NotificationManager {
    cooldowns: HashMap<String, Instant>,
    cooldown_duration: Duration,
    settings: NotificationSettings,
    settings_tx: watch::Sender<NotificationSettings>,
}

impl Default for NotificationManager {
    fn default() -> Self {
        let settings = NotificationSettings::default().normalized();
        let (settings_tx, _settings_rx) = watch::channel(settings.clone());
        Self {
            cooldowns: HashMap::new(),
            cooldown_duration: Duration::from_secs(settings.scan_interval_secs),
            settings,
            settings_tx,
        }
    }
}

impl NotificationManager {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn should_notify(&self, alert_type: &str) -> bool {
        if !self.settings.enabled {
            return false;
        }

        match self.cooldowns.get(alert_type) {
            Some(last_notified_at) => last_notified_at.elapsed() >= self.cooldown_duration,
            None => true,
        }
    }

    pub fn mark_notified(&mut self, alert_type: &str) {
        self.cooldowns
            .insert(alert_type.to_string(), Instant::now());
        self.cooldowns
            .retain(|_, last_notified_at| last_notified_at.elapsed() < self.cooldown_duration);
    }

    pub fn update_settings(&mut self, settings: NotificationSettings) {
        let settings = settings.normalized();
        self.cooldown_duration = Duration::from_secs(settings.scan_interval_secs);
        self.settings = settings;
        let _ = self.settings_tx.send_replace(self.settings.clone());
    }

    #[must_use]
    pub fn settings(&self) -> NotificationSettings {
        self.settings.clone()
    }

    #[must_use]
    pub fn subscribe(&self) -> watch::Receiver<NotificationSettings> {
        self.settings_tx.subscribe()
    }
}

impl NotificationSettings {
    #[must_use]
    pub fn normalized(mut self) -> Self {
        self.scan_interval_secs = self.scan_interval_secs.max(MIN_SCAN_INTERVAL_SECS);
        self
    }
}

#[derive(Debug, Error)]
pub enum NotificationError {
    #[error("通知权限未授予")]
    PermissionDenied,
    #[error("解析通知设置目录失败: {0}")]
    ResolveAppDataDir(String),
    #[error("创建通知设置目录失败: {path}: {source}")]
    CreateDir {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("写入通知设置失败: {path}: {source}")]
    WriteFile {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("序列化通知设置失败: {0}")]
    Serialize(String),
    #[error("桌面通知失败: {0}")]
    Plugin(#[from] tauri_plugin_notification::Error),
}

crate::utils::impl_serialize_display!(NotificationError);

#[must_use]
pub fn create_notification_manager_state() -> NotificationManagerState {
    Arc::new(Mutex::new(NotificationManager::new()))
}

fn notification_settings_file_path(app: &AppHandle) -> Result<PathBuf, NotificationError> {
    app.path()
        .app_data_dir()
        .map(|path| path.join(NOTIFICATION_SETTINGS_FILE_NAME))
        .map_err(|error| NotificationError::ResolveAppDataDir(error.to_string()))
}

pub async fn load_notification_settings(app: &AppHandle) -> NotificationSettings {
    let settings_path = match notification_settings_file_path(app) {
        Ok(path) => path,
        Err(error) => {
            tracing::warn!("解析通知设置路径失败，已回退到默认值: {error}");
            return NotificationSettings::default();
        }
    };

    let settings_content = match tokio::fs::read_to_string(&settings_path).await {
        Ok(content) => content,
        Err(error) if error.kind() == ErrorKind::NotFound => {
            return NotificationSettings::default();
        }
        Err(error) => {
            tracing::warn!(
                "读取通知设置失败，已回退到默认值: path={}, error={error}",
                settings_path.display()
            );
            return NotificationSettings::default();
        }
    };

    match serde_json::from_str::<NotificationSettings>(&settings_content) {
        Ok(settings) => settings.normalized(),
        Err(error) => {
            tracing::warn!(
                "解析通知设置失败，已回退到默认值: path={}, error={error}",
                settings_path.display()
            );
            NotificationSettings::default()
        }
    }
}

pub async fn store_notification_settings(
    app: &AppHandle,
    settings: &NotificationSettings,
) -> Result<(), NotificationError> {
    let settings = settings.clone().normalized();
    let settings_path = notification_settings_file_path(app)?;

    if let Some(parent) = settings_path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|source| NotificationError::CreateDir {
                path: parent.to_string_lossy().into_owned(),
                source,
            })?;
    }

    let payload = serde_json::to_vec_pretty(&settings)
        .map_err(|error| NotificationError::Serialize(error.to_string()))?;

    tokio::fs::write(&settings_path, payload)
        .await
        .map_err(|source| NotificationError::WriteFile {
            path: settings_path.to_string_lossy().into_owned(),
            source,
        })
}

#[must_use]
pub fn alert_type_key(alert_type: &AlertType) -> &'static str {
    match alert_type {
        AlertType::HighTimeoutRate => "high_timeout_rate",
        AlertType::TrafficSurge => "traffic_surge",
        AlertType::TrafficDrop => "traffic_drop",
        AlertType::HighMatchFallback => "high_match_fallback",
        AlertType::DnsFailureCluster => "dns_failure_cluster",
    }
}

pub fn send_desktop_notification(
    app: &AppHandle,
    title: &str,
    body: &str,
) -> Result<(), NotificationError> {
    let notification = app.notification();
    let permission_state = notification.permission_state()?;

    if matches!(
        permission_state,
        PermissionState::Prompt | PermissionState::PromptWithRationale
    ) && notification.request_permission()? != PermissionState::Granted
    {
        return Err(NotificationError::PermissionDenied);
    }

    if permission_state == PermissionState::Denied {
        return Err(NotificationError::PermissionDenied);
    }

    notification.builder().title(title).body(body).show()?;
    Ok(())
}

#[must_use]
pub fn filter_notifiable_alerts(
    manager: &NotificationManager,
    alerts: &[AnomalyAlert],
) -> Vec<AnomalyAlert> {
    let settings = manager.settings();
    let mut seen_alert_types = HashSet::new();

    alerts
        .iter()
        .filter(|alert| is_notifiable_severity(&settings, &alert.severity))
        .filter(|alert| {
            let alert_key = alert_type_key(&alert.alert_type);
            seen_alert_types.insert(alert_key) && manager.should_notify(alert_key)
        })
        .cloned()
        .collect()
}

fn is_notifiable_severity(settings: &NotificationSettings, severity: &AlertSeverity) -> bool {
    match severity {
        AlertSeverity::Info => false,
        AlertSeverity::Critical => true,
        AlertSeverity::Warning => !settings.critical_only,
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::core::anomaly::{AlertSeverity, AlertType, AnomalyAlert};

    fn make_alert(alert_type: AlertType, severity: AlertSeverity) -> AnomalyAlert {
        AnomalyAlert {
            id: format!("alert-{}", alert_type_key(&alert_type)),
            severity,
            alert_type,
            title: "测试告警".to_string(),
            description: "测试描述".to_string(),
            context: json!({}),
            detected_at: "2026-04-09T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn default_settings_use_five_minute_interval() {
        let manager = NotificationManager::new();

        assert_eq!(manager.settings(), NotificationSettings::default());
        assert_eq!(manager.cooldown_duration, Duration::from_secs(300));
    }

    #[test]
    fn update_settings_clamps_interval_to_sixty_seconds() {
        let mut manager = NotificationManager::new();

        manager.update_settings(NotificationSettings {
            enabled: true,
            critical_only: false,
            scan_interval_secs: 15,
        });

        assert_eq!(manager.settings().scan_interval_secs, 60);
        assert_eq!(manager.cooldown_duration, Duration::from_secs(60));
    }

    #[test]
    fn should_notify_respects_cooldown_window() {
        let mut manager = NotificationManager::new();

        assert!(manager.should_notify("high_timeout_rate"));
        manager.mark_notified("high_timeout_rate");
        assert!(!manager.should_notify("high_timeout_rate"));
    }

    #[test]
    fn should_notify_allows_after_cooldown_expires() {
        let mut manager = NotificationManager::new();
        manager.cooldowns.insert(
            "high_timeout_rate".to_string(),
            Instant::now() - Duration::from_secs(301),
        );

        assert!(manager.should_notify("high_timeout_rate"));
    }

    #[test]
    fn filter_notifiable_alerts_excludes_info_alerts() {
        let manager = NotificationManager::new();
        let alerts = vec![
            make_alert(AlertType::HighTimeoutRate, AlertSeverity::Warning),
            make_alert(AlertType::TrafficSurge, AlertSeverity::Info),
        ];

        let filtered = filter_notifiable_alerts(&manager, &alerts);

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].alert_type, AlertType::HighTimeoutRate);
    }

    #[test]
    fn filter_notifiable_alerts_respects_critical_only_setting() {
        let mut manager = NotificationManager::new();
        manager.update_settings(NotificationSettings {
            enabled: true,
            critical_only: true,
            scan_interval_secs: 300,
        });
        let alerts = vec![
            make_alert(AlertType::HighTimeoutRate, AlertSeverity::Warning),
            make_alert(AlertType::DnsFailureCluster, AlertSeverity::Critical),
        ];

        let filtered = filter_notifiable_alerts(&manager, &alerts);

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].alert_type, AlertType::DnsFailureCluster);
    }

    #[test]
    fn filter_notifiable_alerts_deduplicates_alert_types_per_batch() {
        let manager = NotificationManager::new();
        let alerts = vec![
            make_alert(AlertType::HighTimeoutRate, AlertSeverity::Critical),
            make_alert(AlertType::HighTimeoutRate, AlertSeverity::Critical),
        ];

        let filtered = filter_notifiable_alerts(&manager, &alerts);

        assert_eq!(filtered.len(), 1);
    }

    #[test]
    fn mark_notified_uses_stable_alert_type_keys() {
        let mut manager = NotificationManager::new();
        let alert_key = alert_type_key(&AlertType::HighMatchFallback);

        manager.mark_notified(alert_key);

        assert!(manager.cooldowns.contains_key(alert_key));
    }

    #[test]
    fn update_settings_notifies_subscribers() {
        let mut manager = NotificationManager::new();
        let rx = manager.subscribe();

        manager.update_settings(NotificationSettings {
            enabled: false,
            critical_only: true,
            scan_interval_secs: 180,
        });

        assert_eq!(*rx.borrow(), manager.settings());
    }
}
