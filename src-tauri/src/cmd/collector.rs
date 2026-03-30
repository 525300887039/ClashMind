use std::time::Duration;

use chrono::Utc;
use serde::Deserialize;
use tauri::{Listener, Manager, WebviewWindow};
use tokio::sync::{oneshot, watch};

use crate::collector::{ws_client, CollectorError, CollectorState};

const APP_STORE_KEY: &str = "clashmind-store";
const APP_STORE_READ_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, Deserialize)]
struct AppStorePayload {
    #[serde(rename = "apiAddress")]
    api_address: Option<String>,
    #[serde(rename = "apiSecret")]
    api_secret: Option<String>,
    error: Option<String>,
}

#[tauri::command]
pub async fn start_collector(
    window: WebviewWindow,
    state: tauri::State<'_, CollectorState>,
) -> Result<(), CollectorError> {
    let _operation_guard = state.lock_operation().await;
    state.cleanup_finished().await?;
    state.begin_start()?;

    let (api_address, api_secret) = match read_ws_config_from_app_store(&window).await {
        Ok(config) => config,
        Err(error) => {
            let _ = state.abort_start();
            return Err(error);
        }
    };

    let (cancel_tx, cancel_rx) = watch::channel(false);
    let (done_tx, done_rx) = watch::channel(false);
    let task = tauri::async_runtime::spawn(async move {
        ws_client::run_connections_collector(api_address, api_secret, cancel_rx, done_tx).await;
    });

    if let Err(error) = state.start_runtime(cancel_tx, done_rx, task) {
        let _ = state.abort_start();
        return Err(error);
    }

    tracing::info!("collector 启动成功");
    Ok(())
}

#[tauri::command]
pub async fn stop_collector(state: tauri::State<'_, CollectorState>) -> Result<(), CollectorError> {
    let _operation_guard = state.lock_operation().await;
    state.cleanup_finished().await?;
    state.stop_runtime().await?;
    tracing::info!("collector 已停止");
    Ok(())
}

#[tauri::command]
pub async fn get_collector_status(
    state: tauri::State<'_, CollectorState>,
) -> Result<bool, CollectorError> {
    let _operation_guard = state.lock_operation().await;
    state.cleanup_finished().await?;
    Ok(state.is_running())
}

async fn read_ws_config_from_app_store(
    window: &WebviewWindow,
) -> Result<(String, String), CollectorError> {
    let app_handle = window.app_handle().clone();
    let event_name = format!(
        "collector-app-store-{}",
        Utc::now().timestamp_nanos_opt().unwrap_or(0)
    );
    let (tx, rx) = oneshot::channel::<Result<AppStorePayload, CollectorError>>();

    let listener_id = app_handle.once_any(event_name.clone(), move |event| {
        let payload = serde_json::from_str::<AppStorePayload>(event.payload())
            .map_err(|error| CollectorError::StoreReadFailed(format!("payload 无法解析: {error}")))
            .and_then(|payload| match payload.error {
                Some(error) => Err(CollectorError::StoreReadFailed(error)),
                None => Ok(payload),
            });

        let _ = tx.send(payload);
    });

    let script = match build_app_store_bridge_script(&event_name) {
        Ok(script) => script,
        Err(error) => {
            app_handle.unlisten(listener_id);
            return Err(error);
        }
    };

    if let Err(error) = window.eval(script) {
        app_handle.unlisten(listener_id);
        return Err(CollectorError::EvalScript(error.to_string()));
    }

    let payload = match tokio::time::timeout(APP_STORE_READ_TIMEOUT, rx).await {
        Ok(Ok(Ok(payload))) => payload,
        Ok(Ok(Err(error))) => return Err(error),
        Ok(Err(_)) => {
            app_handle.unlisten(listener_id);
            return Err(CollectorError::StoreReadFailed(
                "app-store 回调通道已关闭".into(),
            ));
        }
        Err(_) => {
            app_handle.unlisten(listener_id);
            return Err(CollectorError::StoreReadTimedOut);
        }
    };

    let api_address = payload
        .api_address
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| CollectorError::StoreReadFailed("app-store 缺少 apiAddress".into()))?;
    let api_secret = payload.api_secret.unwrap_or_default();

    Ok((api_address, api_secret))
}

fn build_app_store_bridge_script(event_name: &str) -> Result<String, CollectorError> {
    let serialized_event_name = serde_json::to_string(event_name)
        .map_err(|error| CollectorError::StoreReadFailed(format!("事件名序列化失败: {error}")))?;
    let serialized_store_key = serde_json::to_string(APP_STORE_KEY).map_err(|error| {
        CollectorError::StoreReadFailed(format!("store key 序列化失败: {error}"))
    })?;

    Ok(format!(
        r#"
(() => {{
  const eventName = {serialized_event_name};
  const storeKey = {serialized_store_key};
  const emitPayload = (payload) =>
    window.__TAURI_INTERNALS__.invoke("plugin:event|emit", {{
      event: eventName,
      payload,
    }});

  try {{
    const raw = window.localStorage.getItem(storeKey);
    const parsed = raw ? JSON.parse(raw) : null;
    const state = parsed && typeof parsed === "object" ? (parsed.state ?? parsed) : null;

    if (!state || typeof state !== "object") {{
      void emitPayload({{ error: "app-store unavailable" }});
      return;
    }}

    void emitPayload({{
      apiAddress: typeof state.apiAddress === "string" ? state.apiAddress : null,
      apiSecret: typeof state.apiSecret === "string" ? state.apiSecret : "",
    }});
  }} catch (error) {{
    const message = error instanceof Error ? error.message : String(error);
    void emitPayload({{ error: message }});
  }}
}})();
"#
    ))
}
