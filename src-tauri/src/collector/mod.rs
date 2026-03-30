//! Collector lifecycle state and WebSocket client entry points.

pub mod ws_client;

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

use serde::Serialize;
use tauri::async_runtime::JoinHandle;
use thiserror::Error;
use tokio::sync::watch;

#[derive(Debug)]
struct CollectorRuntime {
    cancel_tx: watch::Sender<bool>,
    task: JoinHandle<()>,
}

/// Shared lifecycle state for the connection collector service.
#[derive(Debug)]
pub struct CollectorState {
    running: Arc<AtomicBool>,
    runtime: Mutex<Option<CollectorRuntime>>,
}

impl CollectorState {
    /// Creates a new idle collector state.
    #[must_use]
    pub fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            runtime: Mutex::new(None),
        }
    }

    /// Returns whether the collector is currently running.
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Returns a clone of the shared running flag for background tasks.
    #[must_use]
    pub fn running_flag(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.running)
    }

    pub(crate) fn start_runtime(
        &self,
        cancel_tx: watch::Sender<bool>,
        task: JoinHandle<()>,
    ) -> Result<(), CollectorError> {
        let mut guard = self
            .runtime
            .lock()
            .map_err(|error| CollectorError::StateLock(error.to_string()))?;

        if guard.is_some() || self.is_running() {
            return Err(CollectorError::AlreadyRunning);
        }

        self.running.store(true, Ordering::SeqCst);
        *guard = Some(CollectorRuntime { cancel_tx, task });
        Ok(())
    }

    pub(crate) async fn cleanup_finished(&self) -> Result<(), CollectorError> {
        let runtime = {
            let mut guard = self
                .runtime
                .lock()
                .map_err(|error| CollectorError::StateLock(error.to_string()))?;

            let should_cleanup = guard
                .as_ref()
                .map(|runtime| runtime.task.inner().is_finished())
                .unwrap_or(false);

            if should_cleanup {
                guard.take()
            } else {
                None
            }
        };

        if let Some(runtime) = runtime {
            self.running.store(false, Ordering::SeqCst);
            runtime
                .task
                .await
                .map_err(|error| CollectorError::TaskJoin(error.to_string()))?;
        }

        Ok(())
    }

    pub(crate) async fn stop_runtime(&self) -> Result<(), CollectorError> {
        let runtime = {
            let mut guard = self
                .runtime
                .lock()
                .map_err(|error| CollectorError::StateLock(error.to_string()))?;

            guard.take().ok_or(CollectorError::NotRunning)?
        };

        let _ = runtime.cancel_tx.send(true);
        self.running.store(false, Ordering::SeqCst);
        runtime
            .task
            .await
            .map_err(|error| CollectorError::TaskJoin(error.to_string()))?;
        Ok(())
    }

    pub(crate) fn request_stop(&self) -> Result<(), CollectorError> {
        let cancel_tx = {
            let guard = self
                .runtime
                .lock()
                .map_err(|error| CollectorError::StateLock(error.to_string()))?;

            guard.as_ref().map(|runtime| runtime.cancel_tx.clone())
        };

        if let Some(cancel_tx) = cancel_tx {
            let _ = cancel_tx.send(true);
        }

        Ok(())
    }
}

impl Default for CollectorState {
    fn default() -> Self {
        Self::new()
    }
}

/// Errors returned by collector commands and background tasks.
#[derive(Debug, Error)]
pub enum CollectorError {
    #[error("采集服务已在运行")]
    AlreadyRunning,
    #[error("采集服务未在运行")]
    NotRunning,
    #[error("采集状态锁失败: {0}")]
    StateLock(String),
    #[error("构建 WebSocket 地址失败: {0}")]
    InvalidApiAddress(String),
    #[error("解析连接快照失败: {0}")]
    SnapshotParse(String),
    #[error("序列化代理链失败: {0}")]
    ProxyChainSerialize(String),
    #[error("执行 app-store 读取脚本失败: {0}")]
    EvalScript(String),
    #[error("读取 app-store 超时")]
    StoreReadTimedOut,
    #[error("读取 app-store 失败: {0}")]
    StoreReadFailed(String),
    #[error("等待采集任务结束失败: {0}")]
    TaskJoin(String),
}

impl Serialize for CollectorError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}
