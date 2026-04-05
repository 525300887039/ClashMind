use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tauri_plugin_shell::process::{CommandChild, CommandEvent};
use tauri_plugin_shell::ShellExt;
use thiserror::Error;
use tokio::sync::oneshot;

const AI_READY_TIMEOUT: Duration = Duration::from_secs(10);
const AI_RPC_TIMEOUT: Duration = Duration::from_secs(10);

enum PendingRpcRequest {
    Unary(oneshot::Sender<Result<serde_json::Value, AiSidecarError>>),
    Stream,
}

type PendingRpcRequests = Arc<Mutex<HashMap<String, PendingRpcRequest>>>;

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
    pub log_task: Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
    pub traffic_task: Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
}

struct AiSidecarRuntime {
    child: CommandChild,
    pending: PendingRpcRequests,
    reader_task: tauri::async_runtime::JoinHandle<()>,
}

pub struct AiSidecarState {
    runtime: Arc<Mutex<Option<AiSidecarRuntime>>>,
    next_request_id: AtomicU64,
}

impl AiSidecarState {
    pub fn new() -> Self {
        Self {
            runtime: Arc::new(Mutex::new(None)),
            next_request_id: AtomicU64::new(1),
        }
    }

    pub fn is_running(&self) -> bool {
        self.runtime
            .lock()
            .map(|runtime| runtime.is_some())
            .unwrap_or(false)
    }
}

impl Default for AiSidecarState {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Error, Debug, Clone)]
pub enum AiSidecarError {
    #[error("ai-service 已在运行")]
    AlreadyRunning,
    #[error("ai-service 未在运行")]
    NotRunning,
    #[error("启动 ai-service 失败: {0}")]
    SpawnFailed(String),
    #[error("停止 ai-service 失败: {0}")]
    KillFailed(String),
    #[error("向 ai-service 写入请求失败: {0}")]
    WriteFailed(String),
    #[error("等待 ai-service ready 超时")]
    ReadyTimeout,
    #[error("ai-service 未发送 ready 消息")]
    ReadySignalDropped,
    #[error("等待 ai-service 响应超时")]
    ResponseTimeout,
    #[error("ai-service 返回无效响应: {0}")]
    InvalidResponse(String),
    #[error("ai-service RPC 错误({code}): {message}")]
    Rpc { code: i64, message: String },
    #[error("ai-service 进程已退出: {0}")]
    ProcessExited(String),
    #[error("ai-service 请求已取消")]
    RequestCancelled,
}

impl Serialize for AiSidecarError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

#[derive(Serialize)]
struct JsonRpcRequest<'a> {
    jsonrpc: &'static str,
    id: String,
    method: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<serde_json::Value>,
}

fn stream_error_payload(error: &AiSidecarError) -> serde_json::Value {
    serde_json::json!({
        "type": "error",
        "message": error.to_string(),
    })
}

fn is_terminal_stream_event(payload: &serde_json::Value) -> bool {
    payload
        .get("type")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|event_type| matches!(event_type, "done" | "error"))
}

fn fail_pending_requests(
    app: Option<&AppHandle>,
    pending: &PendingRpcRequests,
    error: AiSidecarError,
) {
    match pending.lock() {
        Ok(mut pending_requests) => {
            for pending_request in pending_requests.drain().map(|(_, pending_request)| pending_request)
            {
                match pending_request {
                    PendingRpcRequest::Unary(sender) => {
                        let _ = sender.send(Err(error.clone()));
                    }
                    PendingRpcRequest::Stream => {
                        if let Some(app_handle) = app {
                            let _ = app_handle.emit("ai-stream", stream_error_payload(&error));
                        }
                    }
                }
            }
        }
        Err(lock_error) => {
            tracing::error!("清理 ai-service pending 请求失败: {lock_error}");
        }
    }
}

fn clear_ai_runtime(runtime: &Arc<Mutex<Option<AiSidecarRuntime>>>) {
    match runtime.lock() {
        Ok(mut guard) => {
            let _ = guard.take();
        }
        Err(lock_error) => {
            tracing::error!("清理 ai-service 状态失败: {lock_error}");
        }
    }
}

fn extract_response_id(payload: &serde_json::Value) -> Option<String> {
    let id = payload.get("id")?;
    match id {
        serde_json::Value::String(value) => Some(value.clone()),
        serde_json::Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

fn parse_rpc_error(payload: &serde_json::Value) -> AiSidecarError {
    let code = payload
        .get("code")
        .and_then(serde_json::Value::as_i64)
        .unwrap_or(-32603);
    let message = payload
        .get("message")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("unknown rpc error")
        .to_string();

    AiSidecarError::Rpc { code, message }
}

fn handle_ai_stdout(
    app: &AppHandle,
    pending: &PendingRpcRequests,
    ready_sender: &mut Option<oneshot::Sender<Result<(), AiSidecarError>>>,
    line: Vec<u8>,
) {
    let payload = match serde_json::from_slice::<serde_json::Value>(&line) {
        Ok(payload) => payload,
        Err(error) => {
            tracing::warn!(
                "[ai-service] invalid stdout payload: {error}; raw={}",
                String::from_utf8_lossy(&line)
            );
            return;
        }
    };

    if payload
        .get("method")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|method| method == "ready")
    {
        tracing::info!("[ai-service] ready");
        if let Some(sender) = ready_sender.take() {
            let _ = sender.send(Ok(()));
        }
        return;
    }

    if let Some(request_id) = extract_response_id(&payload) {
        enum PendingAction {
            None,
            EmitStream(serde_json::Value),
            EmitStreamError(AiSidecarError),
        }

        let action = match pending.lock() {
            Ok(mut pending_requests) => match pending_requests.remove(&request_id) {
                Some(PendingRpcRequest::Unary(sender)) => {
                    if let Some(error_payload) = payload.get("error") {
                        let error = parse_rpc_error(error_payload);
                        let _ = sender.send(Err(error.clone()));
                        PendingAction::None
                    } else if let Some(result) = payload.get("result") {
                        let _ = sender.send(Ok(result.clone()));
                        PendingAction::None
                    } else {
                        let error = AiSidecarError::InvalidResponse(
                            "missing result or error field".to_string(),
                        );
                        let _ = sender.send(Err(error.clone()));
                        PendingAction::None
                    }
                }
                Some(PendingRpcRequest::Stream) => {
                    if let Some(error_payload) = payload.get("error") {
                        PendingAction::EmitStreamError(parse_rpc_error(error_payload))
                    } else if let Some(result) = payload.get("result") {
                        let should_keep = !is_terminal_stream_event(result);
                        let stream_payload = result.clone();

                        if should_keep {
                            pending_requests.insert(request_id.clone(), PendingRpcRequest::Stream);
                        }

                        PendingAction::EmitStream(stream_payload)
                    } else {
                        PendingAction::EmitStreamError(AiSidecarError::InvalidResponse(
                            "missing result or error field".to_string(),
                        ))
                    }
                }
                None => {
                    tracing::warn!(
                        "[ai-service] received response for unknown request id: {request_id}"
                    );
                    PendingAction::None
                }
            },
            Err(lock_error) => {
                tracing::error!("读取 ai-service pending 请求失败: {lock_error}");
                PendingAction::None
            }
        };

        match action {
            PendingAction::None => {}
            PendingAction::EmitStream(stream_payload) => {
                let _ = app.emit("ai-stream", stream_payload);
            }
            PendingAction::EmitStreamError(error) => {
                let _ = app.emit("ai-stream", stream_error_payload(&error));
            }
        }

        return;
    }

    let _ = app.emit("ai-service-notification", payload);
}

pub async fn start_ai(app: &AppHandle, state: &AiSidecarState) -> Result<(), AiSidecarError> {
    let ready_rx = {
        let mut runtime_guard = state
            .runtime
            .lock()
            .map_err(|error| AiSidecarError::SpawnFailed(error.to_string()))?;

        if runtime_guard.is_some() {
            return Err(AiSidecarError::AlreadyRunning);
        }

        let (mut rx, child) = app
            .shell()
            .sidecar("binaries/ai-service")
            .map_err(|error| AiSidecarError::SpawnFailed(error.to_string()))?
            .spawn()
            .map_err(|error| AiSidecarError::SpawnFailed(error.to_string()))?;

        let pending = Arc::new(Mutex::new(HashMap::new()));
        let runtime_state = Arc::clone(&state.runtime);
        let pending_reader = Arc::clone(&pending);
        let app_handle = app.clone();
        let (ready_tx, ready_rx) = oneshot::channel();

        let reader_task = tauri::async_runtime::spawn(async move {
            let mut ready_sender = Some(ready_tx);

            while let Some(event) = rx.recv().await {
                match event {
                    CommandEvent::Stdout(line) => {
                        handle_ai_stdout(&app_handle, &pending_reader, &mut ready_sender, line);
                    }
                    CommandEvent::Stderr(line) => {
                        tracing::warn!(
                            "[ai-service] {}",
                            String::from_utf8_lossy(&line).trim_end()
                        );
                    }
                    CommandEvent::Error(error) => {
                        tracing::error!("[ai-service] process error: {error}");
                        if let Some(sender) = ready_sender.take() {
                            let _ = sender.send(Err(AiSidecarError::ProcessExited(error.clone())));
                        }
                        fail_pending_requests(
                            Some(&app_handle),
                            &pending_reader,
                            AiSidecarError::ProcessExited(error.clone()),
                        );
                        clear_ai_runtime(&runtime_state);
                        return;
                    }
                    CommandEvent::Terminated(status) => {
                        let reason = format!("code={:?}, signal={:?}", status.code, status.signal);
                        tracing::info!("[ai-service] terminated: {reason}");
                        if let Some(sender) = ready_sender.take() {
                            let _ = sender.send(Err(AiSidecarError::ProcessExited(reason.clone())));
                        }
                        fail_pending_requests(
                            Some(&app_handle),
                            &pending_reader,
                            AiSidecarError::ProcessExited(reason.clone()),
                        );
                        clear_ai_runtime(&runtime_state);
                        return;
                    }
                    _ => {}
                }
            }

            if let Some(sender) = ready_sender.take() {
                let _ = sender.send(Err(AiSidecarError::ReadySignalDropped));
            }
            fail_pending_requests(
                Some(&app_handle),
                &pending_reader,
                AiSidecarError::ProcessExited("stdout channel closed".to_string()),
            );
            clear_ai_runtime(&runtime_state);
        });

        *runtime_guard = Some(AiSidecarRuntime {
            child,
            pending,
            reader_task,
        });

        ready_rx
    };

    match tokio::time::timeout(AI_READY_TIMEOUT, ready_rx).await {
        Ok(Ok(Ok(()))) => Ok(()),
        Ok(Ok(Err(error))) => {
            let _ = stop_ai(Some(app), state);
            Err(error)
        }
        Ok(Err(_)) => {
            let _ = stop_ai(Some(app), state);
            Err(AiSidecarError::ReadySignalDropped)
        }
        Err(_) => {
            let _ = stop_ai(Some(app), state);
            Err(AiSidecarError::ReadyTimeout)
        }
    }
}

pub fn stop_ai(app: Option<&AppHandle>, state: &AiSidecarState) -> Result<(), AiSidecarError> {
    let runtime = state
        .runtime
        .lock()
        .map_err(|error| AiSidecarError::KillFailed(error.to_string()))?
        .take()
        .ok_or(AiSidecarError::NotRunning)?;

    fail_pending_requests(
        app,
        &runtime.pending,
        AiSidecarError::ProcessExited("ai-service 已停止".to_string()),
    );

    let kill_result = runtime
        .child
        .kill()
        .map_err(|error| AiSidecarError::KillFailed(error.to_string()));
    runtime.reader_task.abort();
    kill_result
}

pub fn is_ai_running(state: &AiSidecarState) -> bool {
    state.is_running()
}

pub async fn send_rpc(
    state: &AiSidecarState,
    method: &str,
    params: Option<serde_json::Value>,
) -> Result<serde_json::Value, AiSidecarError> {
    let request_id = state
        .next_request_id
        .fetch_add(1, Ordering::Relaxed)
        .to_string();

    let request = JsonRpcRequest {
        jsonrpc: "2.0",
        id: request_id.clone(),
        method,
        params,
    };
    let payload = format!(
        "{}\n",
        serde_json::to_string(&request)
            .map_err(|error| AiSidecarError::InvalidResponse(error.to_string()))?
    );
    let (response_tx, response_rx) = oneshot::channel();

    {
        let mut runtime_guard = state
            .runtime
            .lock()
            .map_err(|error| AiSidecarError::WriteFailed(error.to_string()))?;
        let runtime = runtime_guard.as_mut().ok_or(AiSidecarError::NotRunning)?;

        runtime
            .pending
            .lock()
            .map_err(|error| AiSidecarError::WriteFailed(error.to_string()))?
            .insert(request_id.clone(), PendingRpcRequest::Unary(response_tx));

        if let Err(error) = runtime.child.write(payload.as_bytes()) {
            if let Ok(mut pending_requests) = runtime.pending.lock() {
                pending_requests.remove(&request_id);
            }
            return Err(AiSidecarError::WriteFailed(error.to_string()));
        }
    }

    match tokio::time::timeout(AI_RPC_TIMEOUT, response_rx).await {
        Ok(Ok(result)) => result,
        Ok(Err(_)) => Err(AiSidecarError::RequestCancelled),
        Err(_) => {
            if let Ok(mut runtime_guard) = state.runtime.lock() {
                if let Some(runtime) = runtime_guard.as_mut() {
                    if let Ok(mut pending_requests) = runtime.pending.lock() {
                        pending_requests.remove(&request_id);
                    }
                }
            }
            Err(AiSidecarError::ResponseTimeout)
        }
    }
}

pub fn send_streaming_rpc(
    state: &AiSidecarState,
    method: &str,
    params: Option<serde_json::Value>,
) -> Result<(), AiSidecarError> {
    let request_id = state
        .next_request_id
        .fetch_add(1, Ordering::Relaxed)
        .to_string();

    let request = JsonRpcRequest {
        jsonrpc: "2.0",
        id: request_id.clone(),
        method,
        params,
    };
    let payload = format!(
        "{}\n",
        serde_json::to_string(&request)
            .map_err(|error| AiSidecarError::InvalidResponse(error.to_string()))?
    );

    let mut runtime_guard = state
        .runtime
        .lock()
        .map_err(|error| AiSidecarError::WriteFailed(error.to_string()))?;
    let runtime = runtime_guard.as_mut().ok_or(AiSidecarError::NotRunning)?;

    runtime
        .pending
        .lock()
        .map_err(|error| AiSidecarError::WriteFailed(error.to_string()))?
        .insert(request_id.clone(), PendingRpcRequest::Stream);

    if let Err(error) = runtime.child.write(payload.as_bytes()) {
        if let Ok(mut pending_requests) = runtime.pending.lock() {
            pending_requests.remove(&request_id);
        }
        return Err(AiSidecarError::WriteFailed(error.to_string()));
    }

    Ok(())
}

pub fn start(app: &AppHandle, state: &SidecarState, config_path: &str) -> Result<(), SidecarError> {
    let mut child_lock = state
        .child
        .lock()
        .map_err(|e| SidecarError::SpawnFailed(e.to_string()))?;

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

/// Best-effort abort of log/traffic subscription tasks.
pub fn abort_subscriptions(state: &SidecarState) {
    if let Ok(mut guard) = state.log_task.lock() {
        if let Some(h) = guard.take() {
            h.abort();
        }
    }
    if let Ok(mut guard) = state.traffic_task.lock() {
        if let Some(h) = guard.take() {
            h.abort();
        }
    }
}

pub fn stop(state: &SidecarState) -> Result<(), SidecarError> {
    abort_subscriptions(state);

    let mut child_lock = state
        .child
        .lock()
        .map_err(|e| SidecarError::KillFailed(e.to_string()))?;

    match child_lock.take() {
        Some(child) => child
            .kill()
            .map_err(|e| SidecarError::KillFailed(e.to_string())),
        None => Err(SidecarError::NotRunning),
    }
}

pub fn restart(
    app: &AppHandle,
    state: &SidecarState,
    config_path: &str,
) -> Result<(), SidecarError> {
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
