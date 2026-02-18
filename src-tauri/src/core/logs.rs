use futures_util::StreamExt;
use tauri::{AppHandle, Emitter};
use tokio_tungstenite::connect_async;
use tracing::{error, info};

#[derive(Clone, serde::Serialize)]
struct LogPayload {
    #[serde(rename = "type")]
    log_type: String,
    payload: String,
}

pub fn start_log_subscription(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut retry_delay = std::time::Duration::from_secs(1);
        let max_delay = std::time::Duration::from_secs(30);

        loop {
            let url = "ws://127.0.0.1:9090/logs?level=debug";
            match connect_async(url).await {
                Ok((ws, _)) => {
                    info!("日志 WebSocket 已连接");
                    retry_delay = std::time::Duration::from_secs(1);
                    let (_, mut read) = ws.split();

                    while let Some(msg) = read.next().await {
                        match msg {
                            Ok(tokio_tungstenite::tungstenite::Message::Text(text)) => {
                                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&text) {
                                    let payload = LogPayload {
                                        log_type: v["type"].as_str().unwrap_or("info").to_string(),
                                        payload: v["payload"].as_str().unwrap_or("").to_string(),
                                    };
                                    let _ = app.emit("log-update", payload);
                                }
                            }
                            Err(e) => {
                                error!("日志 WebSocket 错误: {e}");
                                break;
                            }
                            _ => {}
                        }
                    }
                }
                Err(e) => {
                    error!("日志 WebSocket 连接失败: {e}");
                }
            }

            tokio::time::sleep(retry_delay).await;
            retry_delay = (retry_delay * 2).min(max_delay);
        }
    });
}
