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
    state.cleanup_finished().await?;

    if state.is_running() {
        return Err(CollectorError::AlreadyRunning);
    }

    let (api_address, api_secret) = read_ws_config_from_app_store(&window).await?;
    let running = state.running_flag();
    let (cancel_tx, cancel_rx) = watch::channel(false);
    let task = tauri::async_runtime::spawn(async move {
        ws_client::run_connections_collector(api_address, api_secret, running, cancel_rx).await;
    });

    state.start_runtime(cancel_tx, task)?;
    tracing::info!("collector 启动成功");
    Ok(())
}

#[tauri::command]
pub async fn stop_collector(state: tauri::State<'_, CollectorState>) -> Result<(), CollectorError> {
    state.stop_runtime().await?;
    tracing::info!("collector 已停止");
    Ok(())
}

#[tauri::command]
pub async fn get_collector_status(
    state: tauri::State<'_, CollectorState>,
) -> Result<bool, CollectorError> {
    state.cleanup_finished().await?;
    Ok(state.is_running())
}

async fn read_ws_config_from_app_store(
    window: &WebviewWindow,
) -> Result<(String, String), CollectorError> {
    let event_name = format!(
        "collector-app-store-{}",
        Utc::now().timestamp_nanos_opt().unwrap_or(0)
    );
    let (tx, rx) = oneshot::channel::<Result<AppStorePayload, CollectorError>>();

    window
        .app_handle()
        .once_any(event_name.clone(), move |event| {
            let payload = serde_json::from_str::<AppStorePayload>(event.payload())
                .map_err(|error| {
                    CollectorError::StoreReadFailed(format!("payload 无法解析: {error}"))
                })
                .and_then(|payload| match payload.error {
                    Some(error) => Err(CollectorError::StoreReadFailed(error)),
                    None => Ok(payload),
                });

            let _ = tx.send(payload);
        });

    let script = build_app_store_bridge_script(&event_name)?;
    window
        .eval(script)
        .map_err(|error| CollectorError::EvalScript(error.to_string()))?;

    let payload = tokio::time::timeout(APP_STORE_READ_TIMEOUT, rx)
        .await
        .map_err(|_| CollectorError::StoreReadTimedOut)?
        .map_err(|_| CollectorError::StoreReadFailed("app-store 回调通道已关闭".into()))??;

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
