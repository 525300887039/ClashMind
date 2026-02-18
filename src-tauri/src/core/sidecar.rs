use std::sync::Mutex;

use serde::Serialize;
use tauri::AppHandle;
use tauri_plugin_shell::ShellExt;
use tauri_plugin_shell::process::{CommandChild, CommandEvent};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SidecarError {
    #[error("mihomo 已在运行")]
    AlreadyRunning,
    #[error("mihomo 未在运行")]
    NotRunning,
    #[error("启动 mihomo 失败: {0}")]
    SpawnFailed(String),
    #[error("停止 mihomo 失败: {0}")]
    KillFailed(String),
}

impl Serialize for SidecarError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

pub struct SidecarState {
    pub child: Mutex<Option<CommandChild>>,
}

pub fn start(app: &AppHandle, state: &SidecarState, config_path: &str) -> Result<(), SidecarError> {
    let mut child_lock = state.child.lock().map_err(|e| SidecarError::SpawnFailed(e.to_string()))?;

    if child_lock.is_some() {
        return Err(SidecarError::AlreadyRunning);
    }

    let (mut rx, child) = app
        .shell()
        .sidecar("binaries/mihomo")
        .map_err(|e| SidecarError::SpawnFailed(e.to_string()))?
        .args(["-d", config_path])
        .spawn()
        .map_err(|e| SidecarError::SpawnFailed(e.to_string()))?;

    tauri::async_runtime::spawn(async move {
        while let Some(event) = rx.recv().await {
            match event {
                CommandEvent::Stdout(line) => {
                    tracing::info!("[mihomo] {}", String::from_utf8_lossy(&line));
                }
                CommandEvent::Stderr(line) => {
                    tracing::warn!("[mihomo] {}", String::from_utf8_lossy(&line));
                }
                CommandEvent::Terminated(status) => {
                    tracing::info!("[mihomo] terminated: {:?}", status);
                    break;
                }
                _ => {}
            }
        }
    });

    *child_lock = Some(child);
    Ok(())
}

pub fn stop(state: &SidecarState) -> Result<(), SidecarError> {
    let mut child_lock = state.child.lock().map_err(|e| SidecarError::KillFailed(e.to_string()))?;

    match child_lock.take() {
        Some(child) => child.kill().map_err(|e| SidecarError::KillFailed(e.to_string())),
        None => Err(SidecarError::NotRunning),
    }
}

pub fn restart(app: &AppHandle, state: &SidecarState, config_path: &str) -> Result<(), SidecarError> {
    // 如果正在运行则先停止，忽略 NotRunning 错误
    match stop(state) {
        Ok(()) | Err(SidecarError::NotRunning) => {}
        Err(e) => return Err(e),
    }
    start(app, state, config_path)
}

pub fn is_running(state: &SidecarState) -> bool {
    state
        .child
        .lock()
        .map(|lock| lock.is_some())
        .unwrap_or(false)
}
