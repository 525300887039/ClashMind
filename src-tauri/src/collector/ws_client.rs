use std::{collections::HashMap, time::Duration};

use chrono::Utc;
use futures_util::StreamExt;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use tokio::sync::watch;
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};
use tracing::{info, warn};

use super::CollectorError;

const INITIAL_RETRY_DELAY: Duration = Duration::from_secs(1);
const MAX_RETRY_DELAY: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConnectionRecord {
    pub id: String,
    pub host: String,
    pub dst_ip: Option<String>,
    pub dst_port: Option<i32>,
    pub src_ip: Option<String>,
    pub src_port: Option<i32>,
    pub network: String,
    pub conn_type: String,
    pub rule: String,
    pub rule_payload: Option<String>,
    pub proxy_chain: String,
    pub upload: i64,
    pub download: i64,
    pub start_time: String,
}

#[derive(Debug, Deserialize)]
struct ConnectionsSnapshot {
    #[serde(default)]
    connections: Vec<RawConnection>,
}

#[derive(Debug, Deserialize)]
struct RawConnection {
    id: String,
    metadata: RawMetadata,
    #[serde(default)]
    upload: i64,
    #[serde(default)]
    download: i64,
    #[serde(default)]
    start: String,
    #[serde(default)]
    chains: Vec<String>,
    #[serde(default)]
    rule: String,
    #[serde(rename = "rulePayload")]
    rule_payload: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct RawMetadata {
    #[serde(default)]
    host: String,
    #[serde(rename = "destinationIP")]
    destination_ip: Option<String>,
    #[serde(rename = "destinationPort")]
    destination_port: Option<String>,
    #[serde(rename = "sourceIP")]
    source_ip: Option<String>,
    #[serde(rename = "sourcePort")]
    source_port: Option<String>,
    #[serde(default)]
    network: String,
    #[serde(rename = "type", default)]
    conn_type: String,
}

#[derive(Debug, Default)]
struct SnapshotDiff {
    opened: Vec<ConnectionRecord>,
    updated: Vec<ConnectionRecord>,
    closed: Vec<ClosedConnectionRecord>,
}

#[derive(Debug)]
struct ClosedConnectionRecord {
    id: String,
    host: String,
    close_time: String,
}

enum ConnectionLoopControl {
    Stop,
    Reconnect,
}

impl RawConnection {
    fn into_record(self) -> Result<ConnectionRecord, CollectorError> {
        let dst_port = parse_port(self.metadata.destination_port.as_deref());
        let src_port = parse_port(self.metadata.source_port.as_deref());
        let proxy_chain = serde_json::to_string(&self.chains)
            .map_err(|error| CollectorError::ProxyChainSerialize(error.to_string()))?;

        Ok(ConnectionRecord {
            id: self.id,
            host: self.metadata.host,
            dst_ip: self.metadata.destination_ip,
            dst_port,
            src_ip: self.metadata.source_ip,
            src_port,
            network: self.metadata.network,
            conn_type: self.metadata.conn_type,
            rule: self.rule,
            rule_payload: self.rule_payload.filter(|value| !value.is_empty()),
            proxy_chain,
            upload: self.upload,
            download: self.download,
            start_time: self.start,
        })
    }
}

impl SnapshotDiff {
    fn is_empty(&self) -> bool {
        self.opened.is_empty() && self.updated.is_empty() && self.closed.is_empty()
    }
}

/// Runs the mihomo `/connections` collector loop until a shutdown signal is received.
pub async fn run_connections_collector(
    api_address: String,
    api_secret: String,
    mut shutdown_rx: watch::Receiver<bool>,
    done_tx: watch::Sender<bool>,
) {
    let mut previous_connections = HashMap::new();
    let mut retry_delay = INITIAL_RETRY_DELAY;

    loop {
        if should_stop(&shutdown_rx) {
            break;
        }

        let ws_url = match build_connections_ws_url(&api_address, &api_secret) {
            Ok(url) => url,
            Err(error) => {
                warn!("collector 无法构建 WebSocket 地址: {error}");
                if wait_for_retry_or_stop(&mut shutdown_rx, retry_delay).await {
                    break;
                }
                retry_delay = next_retry_delay(retry_delay);
                continue;
            }
        };

        let connect_future = connect_async(ws_url.as_str());
        tokio::pin!(connect_future);

        let connection = tokio::select! {
            changed = shutdown_rx.changed() => {
                if changed.is_err() || should_stop(&shutdown_rx) {
                    break;
                }
                continue;
            }
            result = &mut connect_future => result,
        };

        match connection {
            Ok((websocket, _)) => {
                info!(api_address = %api_address, "collector 已连接 mihomo /connections");
                retry_delay = INITIAL_RETRY_DELAY;

                match collect_stream(websocket, &mut previous_connections, &mut shutdown_rx).await {
                    ConnectionLoopControl::Stop => break,
                    ConnectionLoopControl::Reconnect => {}
                }
            }
            Err(error) => {
                warn!("collector 连接 mihomo /connections 失败: {error}");
            }
        }

        if wait_for_retry_or_stop(&mut shutdown_rx, retry_delay).await {
            break;
        }
        retry_delay = next_retry_delay(retry_delay);
    }

    let _ = done_tx.send(true);
    info!("collector 已停止");
}

fn build_connections_ws_url(api_address: &str, api_secret: &str) -> Result<String, CollectorError> {
    let trimmed_address = api_address.trim();
    if trimmed_address.is_empty() {
        return Err(CollectorError::InvalidApiAddress("apiAddress 为空".into()));
    }

    let normalized = if has_supported_scheme(trimmed_address) {
        trimmed_address.to_string()
    } else {
        format!("http://{trimmed_address}")
    };

    let mut url = Url::parse(&normalized)
        .map_err(|error| CollectorError::InvalidApiAddress(error.to_string()))?;

    let ws_scheme = match url.scheme() {
        "http" => "ws",
        "https" => "wss",
        "ws" => "ws",
        "wss" => "wss",
        other => {
            return Err(CollectorError::InvalidApiAddress(format!(
                "不支持的协议: {other}"
            )));
        }
    };

    url.set_scheme(ws_scheme)
        .map_err(|_| CollectorError::InvalidApiAddress("无法设置 WebSocket 协议".into()))?;
    url.set_path("/connections");
    url.set_query(None);

    if !api_secret.trim().is_empty() {
        url.query_pairs_mut()
            .append_pair("token", api_secret.trim());
    }

    Ok(url.to_string())
}

async fn collect_stream(
    websocket: WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>,
    previous_connections: &mut HashMap<String, ConnectionRecord>,
    shutdown_rx: &mut watch::Receiver<bool>,
) -> ConnectionLoopControl {
    let (_, mut read) = websocket.split();

    loop {
        let message = tokio::select! {
            changed = shutdown_rx.changed() => {
                if changed.is_err() || should_stop(shutdown_rx) {
                    info!("collector 收到停止信号");
                    return ConnectionLoopControl::Stop;
                }
                continue;
            }
            message = read.next() => message,
        };

        match message {
            Some(Ok(Message::Text(text))) => {
                if let Err(error) = apply_snapshot(text.as_ref(), previous_connections) {
                    warn!("collector 处理连接快照失败: {error}");
                }
            }
            Some(Ok(Message::Close(frame))) => {
                info!(?frame, "collector WebSocket 已关闭");
                return ConnectionLoopControl::Reconnect;
            }
            Some(Ok(_)) => {}
            Some(Err(error)) => {
                warn!("collector WebSocket 读取失败: {error}");
                return ConnectionLoopControl::Reconnect;
            }
            None => {
                warn!("collector WebSocket 已断开");
                return ConnectionLoopControl::Reconnect;
            }
        }
    }
}

fn apply_snapshot(
    payload: &str,
    previous_connections: &mut HashMap<String, ConnectionRecord>,
) -> Result<(), CollectorError> {
    let snapshot: ConnectionsSnapshot = serde_json::from_str(payload)
        .map_err(|error| CollectorError::SnapshotParse(error.to_string()))?;
    let mut current_connections = HashMap::with_capacity(snapshot.connections.len());

    for raw_connection in snapshot.connections {
        let record = raw_connection.into_record()?;
        current_connections.insert(record.id.clone(), record);
    }

    let diff = diff_snapshots(previous_connections, &current_connections);
    if !diff.is_empty() {
        log_snapshot_diff(&diff, current_connections.len());
    }

    *previous_connections = current_connections;
    Ok(())
}

fn diff_snapshots(
    previous_connections: &HashMap<String, ConnectionRecord>,
    current_connections: &HashMap<String, ConnectionRecord>,
) -> SnapshotDiff {
    let mut diff = SnapshotDiff::default();

    for (connection_id, current_record) in current_connections {
        match previous_connections.get(connection_id) {
            Some(previous_record) if previous_record != current_record => {
                diff.updated.push(current_record.clone());
            }
            None => diff.opened.push(current_record.clone()),
            _ => {}
        }
    }

    for (connection_id, previous_record) in previous_connections {
        if !current_connections.contains_key(connection_id) {
            diff.closed.push(ClosedConnectionRecord {
                id: connection_id.clone(),
                host: previous_record.host.clone(),
                close_time: Utc::now().to_rfc3339(),
            });
        }
    }

    diff
}

fn log_snapshot_diff(diff: &SnapshotDiff, active_connections: usize) {
    info!(
        opened = diff.opened.len(),
        updated = diff.updated.len(),
        closed = diff.closed.len(),
        active_connections,
        "collector 检测到连接变化"
    );

    for record in &diff.opened {
        info!(
            id = %record.id,
            host = %record.host,
            upload = record.upload,
            download = record.download,
            "collector 新增连接"
        );
    }

    for record in &diff.updated {
        info!(
            id = %record.id,
            host = %record.host,
            upload = record.upload,
            download = record.download,
            "collector 更新连接"
        );
    }

    for record in &diff.closed {
        info!(
            id = %record.id,
            host = %record.host,
            close_time = %record.close_time,
            "collector 关闭连接"
        );
    }
}

async fn wait_for_retry_or_stop(shutdown_rx: &mut watch::Receiver<bool>, delay: Duration) -> bool {
    tokio::select! {
        changed = shutdown_rx.changed() => changed.is_err() || should_stop(shutdown_rx),
        _ = tokio::time::sleep(delay) => false,
    }
}

fn should_stop(shutdown_rx: &watch::Receiver<bool>) -> bool {
    *shutdown_rx.borrow()
}

fn next_retry_delay(current_delay: Duration) -> Duration {
    (current_delay * 2).min(MAX_RETRY_DELAY)
}

fn parse_port(port: Option<&str>) -> Option<i32> {
    port.and_then(|value| value.trim().parse::<i32>().ok())
}

fn has_supported_scheme(value: &str) -> bool {
    value.starts_with("http://")
        || value.starts_with("https://")
        || value.starts_with("ws://")
        || value.starts_with("wss://")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_record(id: &str, host: &str, upload: i64, download: i64) -> ConnectionRecord {
        ConnectionRecord {
            id: id.to_string(),
            host: host.to_string(),
            dst_ip: Some("1.1.1.1".into()),
            dst_port: Some(443),
            src_ip: Some("192.168.1.2".into()),
            src_port: Some(12345),
            network: "tcp".into(),
            conn_type: "HTTPS".into(),
            rule: "DOMAIN-SUFFIX".into(),
            rule_payload: Some(host.to_string()),
            proxy_chain: "[\"DIRECT\"]".into(),
            upload,
            download,
            start_time: "2026-03-30T10:00:00.000Z".into(),
        }
    }

    #[test]
    fn build_connections_ws_url_supports_plain_addresses() {
        let result = build_connections_ws_url("127.0.0.1:9090", "secret token");

        assert!(result.is_ok());
        let Ok(url) = result else {
            panic!("expected a valid websocket url");
        };

        assert_eq!(url, "ws://127.0.0.1:9090/connections?token=secret+token");
    }

    #[test]
    fn build_connections_ws_url_converts_https_to_wss() {
        let result = build_connections_ws_url("https://example.com/api", "");

        assert!(result.is_ok());
        let Ok(url) = result else {
            panic!("expected a valid websocket url");
        };

        assert_eq!(url, "wss://example.com/connections");
    }

    #[test]
    fn diff_snapshots_detects_opened_updated_and_closed_connections() {
        let previous_connections = HashMap::from([
            (
                "kept".to_string(),
                sample_record("kept", "old.example", 1, 1),
            ),
            (
                "closed".to_string(),
                sample_record("closed", "closed.example", 2, 2),
            ),
        ]);
        let current_connections = HashMap::from([
            (
                "kept".to_string(),
                sample_record("kept", "old.example", 3, 4),
            ),
            (
                "opened".to_string(),
                sample_record("opened", "new.example", 5, 6),
            ),
        ]);

        let diff = diff_snapshots(&previous_connections, &current_connections);

        assert_eq!(diff.opened.len(), 1);
        assert_eq!(diff.updated.len(), 1);
        assert_eq!(diff.closed.len(), 1);
        assert_eq!(diff.opened[0].id, "opened");
        assert_eq!(diff.updated[0].id, "kept");
        assert_eq!(diff.closed[0].id, "closed");
    }
}
