use futures_util::StreamExt;
use tauri::{AppHandle, Emitter};
use tokio_tungstenite::connect_async;
use tracing::{error, info};

#[derive(Clone, serde::Serialize)]
struct TrafficPayload {
    up: u64,
    down: u64,
}

pub fn start_traffic_subscription(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut retry_delay = std::time::Duration::from_secs(1);
        let max_delay = std::time::Duration::from_secs(30);

        loop {
            let url = "ws://127.0.0.1:9090/traffic";
            match connect_async(url).await {
                Ok((ws, _)) => {
                    info!("流量 WebSocket 已连接");
                    retry_delay = std::time::Duration::from_secs(1);
                    let (_, mut read) = ws.split();

                    while let Some(msg) = read.next().await {
                        match msg {
                            Ok(tokio_tungstenite::tungstenite::Message::Text(text)) => {
                                if let Ok(v) =
                                    serde_json::from_str::<serde_json::Value>(&text)
                                {
                                    let payload = TrafficPayload {
                                        up: v["up"].as_u64().unwrap_or(0),
                                        down: v["down"].as_u64().unwrap_or(0),
                                    };
                                    let _ = app.emit("traffic-update", payload);
                                }
                            }
                            Err(e) => {
                                error!("流量 WebSocket 错误: {e}");
                                break;
                            }
                            _ => {}
                        }
                    }
                }
                Err(e) => {
                    error!("流量 WebSocket 连接失败: {e}");
                }
            }

            tokio::time::sleep(retry_delay).await;
            retry_delay = (retry_delay * 2).min(max_delay);
        }
    });
}
