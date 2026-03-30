//! Collector lifecycle state and WebSocket client entry points.

pub mod buffer;
pub mod ws_client;

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

use serde::Serialize;
use tauri::async_runtime::JoinHandle;
use thiserror::Error;
use tokio::sync::watch;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum CollectorShutdown {
    #[default]
    Run,
    Stop,
    StopAndCloseActive,
}

impl CollectorShutdown {
    #[must_use]
    pub(crate) fn should_stop(self) -> bool {
        !matches!(self, Self::Run)
    }

    #[must_use]
    pub(crate) fn should_close_active(self) -> bool {
        matches!(self, Self::StopAndCloseActive)
    }
}

#[derive(Debug)]
struct CollectorRuntime {
    cancel_tx: watch::Sender<CollectorShutdown>,
    done_rx: watch::Receiver<bool>,
    task: JoinHandle<()>,
}

#[derive(Debug, Default)]
enum CollectorLifecycle {
    #[default]
    Idle,
    Starting,
    Running(CollectorRuntime),
    Stopping(CollectorRuntime),
}

/// Shared lifecycle state for the connection collector service.
#[derive(Debug)]
pub struct CollectorState {
    running: Arc<AtomicBool>,
    operation: tokio::sync::Mutex<()>,
    lifecycle: Mutex<CollectorLifecycle>,
}

impl CollectorState {
    /// Creates a new idle collector state.
    #[must_use]
    pub fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            operation: tokio::sync::Mutex::new(()),
            lifecycle: Mutex::new(CollectorLifecycle::Idle),
        }
    }

    /// Returns whether the collector is currently running.
    #[must_use]
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    pub(crate) async fn lock_operation(&self) -> tokio::sync::MutexGuard<'_, ()> {
        self.operation.lock().await
    }

    pub(crate) fn begin_start(&self) -> Result<(), CollectorError> {
        let mut guard = self
            .lifecycle
            .lock()
            .map_err(|error| CollectorError::StateLock(error.to_string()))?;

        match &*guard {
            CollectorLifecycle::Idle => {
                *guard = CollectorLifecycle::Starting;
                Ok(())
            }
            CollectorLifecycle::Starting
            | CollectorLifecycle::Running(_)
            | CollectorLifecycle::Stopping(_) => Err(CollectorError::AlreadyRunning),
        }
    }

    pub(crate) fn abort_start(&self) -> Result<(), CollectorError> {
        let mut guard = self
            .lifecycle
            .lock()
            .map_err(|error| CollectorError::StateLock(error.to_string()))?;

        if matches!(&*guard, CollectorLifecycle::Starting) {
            *guard = CollectorLifecycle::Idle;
        }

        Ok(())
    }

    pub(crate) fn start_runtime(
        &self,
        cancel_tx: watch::Sender<CollectorShutdown>,
        done_rx: watch::Receiver<bool>,
        task: JoinHandle<()>,
    ) -> Result<(), CollectorError> {
        let mut guard = self
            .lifecycle
            .lock()
            .map_err(|error| CollectorError::StateLock(error.to_string()))?;

        match &*guard {
            CollectorLifecycle::Starting => {
                self.running.store(true, Ordering::SeqCst);
                *guard = CollectorLifecycle::Running(CollectorRuntime {
                    cancel_tx,
                    done_rx,
                    task,
                });
                Ok(())
            }
            _ => Err(CollectorError::StateTransition(
                "collector 未处于可启动状态".into(),
            )),
        }
    }

    pub(crate) async fn cleanup_finished(&self) -> Result<(), CollectorError> {
        if let Some(runtime) = self.take_finished_runtime()? {
            self.running.store(false, Ordering::SeqCst);
            runtime
                .task
                .await
                .map_err(|error| CollectorError::TaskJoin(error.to_string()))?;
        }

        Ok(())
    }

    pub(crate) async fn stop_runtime(
        &self,
        shutdown: CollectorShutdown,
    ) -> Result<(), CollectorError> {
        let mut done_rx = {
            let mut guard = self
                .lifecycle
                .lock()
                .map_err(|error| CollectorError::StateLock(error.to_string()))?;

            let current = std::mem::take(&mut *guard);
            match current {
                CollectorLifecycle::Running(runtime) => {
                    let _ = runtime.cancel_tx.send(shutdown);
                    let done_rx = runtime.done_rx.clone();
                    *guard = CollectorLifecycle::Stopping(runtime);
                    done_rx
                }
                CollectorLifecycle::Stopping(runtime) => {
                    let _ = runtime.cancel_tx.send(shutdown);
                    let done_rx = runtime.done_rx.clone();
                    *guard = CollectorLifecycle::Stopping(runtime);
                    done_rx
                }
                other => {
                    *guard = other;
                    return Err(CollectorError::NotRunning);
                }
            }
        };

        if !*done_rx.borrow() {
            let _ = done_rx.changed().await;
        }

        self.cleanup_finished().await?;
        Ok(())
    }

    fn take_finished_runtime(&self) -> Result<Option<CollectorRuntime>, CollectorError> {
        let mut guard = self
            .lifecycle
            .lock()
            .map_err(|error| CollectorError::StateLock(error.to_string()))?;

        let current = std::mem::take(&mut *guard);
        let runtime = match current {
            CollectorLifecycle::Running(runtime) if runtime.is_finished() => Some(runtime),
            CollectorLifecycle::Stopping(runtime) if runtime.is_finished() => Some(runtime),
            other => {
                *guard = other;
                None
            }
        };

        if runtime.is_none() && matches!(&*guard, CollectorLifecycle::Idle) {
            *guard = CollectorLifecycle::Idle;
        }

        Ok(runtime)
    }
}

impl CollectorRuntime {
    fn is_finished(&self) -> bool {
        *self.done_rx.borrow() || self.task.inner().is_finished()
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
    #[error("采集状态转换失败: {0}")]
    StateTransition(String),
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

#[cfg(test)]
mod tests {
    use std::{sync::Arc, time::Duration};

    use super::*;

    #[test]
    fn begin_start_rejects_duplicate_start_attempts() {
        let state = CollectorState::new();

        assert!(state.begin_start().is_ok());
        assert!(matches!(
            state.begin_start(),
            Err(CollectorError::AlreadyRunning)
        ));
        assert!(state.abort_start().is_ok());
        assert!(state.begin_start().is_ok());
    }

    #[tokio::test]
    async fn stop_runtime_keeps_lifecycle_busy_until_task_finishes() {
        let state = Arc::new(CollectorState::new());

        assert!(state.begin_start().is_ok());

        let (cancel_tx, mut cancel_rx) = watch::channel(CollectorShutdown::Run);
        let (done_tx, done_rx) = watch::channel(false);
        let task = tauri::async_runtime::spawn(async move {
            let _ = cancel_rx.changed().await;
            tokio::time::sleep(Duration::from_millis(20)).await;
            let _ = done_tx.send(true);
        });

        assert!(state.start_runtime(cancel_tx, done_rx, task).is_ok());

        let stop_state = Arc::clone(&state);
        let stop_task =
            tokio::spawn(async move { stop_state.stop_runtime(CollectorShutdown::Stop).await });

        tokio::time::sleep(Duration::from_millis(5)).await;
        assert!(matches!(
            state.begin_start(),
            Err(CollectorError::AlreadyRunning)
        ));

        let stop_result = stop_task.await;
        assert!(stop_result.is_ok());
        if let Ok(result) = stop_result {
            assert!(result.is_ok());
        }

        assert!(!state.is_running());
        assert!(state.begin_start().is_ok());
    }

    #[test]
    fn shutdown_signal_flags_close_active_only_for_app_exit() {
        assert!(!CollectorShutdown::Run.should_stop());
        assert!(!CollectorShutdown::Run.should_close_active());
        assert!(CollectorShutdown::Stop.should_stop());
        assert!(!CollectorShutdown::Stop.should_close_active());
        assert!(CollectorShutdown::StopAndCloseActive.should_stop());
        assert!(CollectorShutdown::StopAndCloseActive.should_close_active());
    }
}
