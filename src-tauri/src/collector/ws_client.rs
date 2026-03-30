use std::{collections::HashMap, time::Duration};

use chrono::{DateTime, Utc};
use futures_util::StreamExt;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, Runtime};
use tokio::{
    sync::watch,
    time::{self, MissedTickBehavior},
};
use tokio_tungstenite::{connect_async, tungstenite::Message, MaybeTlsStream, WebSocketStream};
use tracing::{info, warn};

use crate::{
    collector::{
        buffer::{BatchBuffer, DEFAULT_BATCH_CAPACITY, DEFAULT_FLUSH_INTERVAL},
        RealtimeStore,
    },
    db::{self, repo_connection, repo_domain::DomainStatsUpdate, DbError},
};

use super::{CollectorError, CollectorShutdown};

const INITIAL_RETRY_DELAY: Duration = Duration::from_secs(1);
const MAX_RETRY_DELAY: Duration = Duration::from_secs(30);
const FLUSH_CHECK_INTERVAL: Duration = Duration::from_secs(1);

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
    updated: Vec<UpdatedConnectionRecord>,
    closed: Vec<ClosedConnectionRecord>,
}

#[derive(Debug)]
struct UpdatedConnectionRecord {
    current: ConnectionRecord,
    previous: ConnectionRecord,
}

#[derive(Debug, Clone)]
struct ClosedConnectionRecord {
    id: String,
    host: String,
    close_time: String,
}

enum ConnectionLoopControl {
    Stop,
    Reconnect,
}

type PendingDomainStats = HashMap<(String, String), DomainStatsUpdate>;
type PendingCloseUpdates = HashMap<String, ClosedConnectionRecord>;

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
pub async fn run_connections_collector<R: Runtime>(
    app_handle: AppHandle<R>,
    api_address: String,
    api_secret: String,
    mut shutdown_rx: watch::Receiver<CollectorShutdown>,
    done_tx: watch::Sender<bool>,
) {
    let mut previous_connections = match load_open_connections_from_db(&app_handle).await {
        Ok(records) => records,
        Err(error) => {
            warn!("collector 恢复打开连接基线失败: {error}");
            HashMap::new()
        }
    };
    let mut batch_buffer = BatchBuffer::new(DEFAULT_BATCH_CAPACITY, DEFAULT_FLUSH_INTERVAL);
    let mut pending_domain_stats = PendingDomainStats::new();
    let mut pending_close_updates = PendingCloseUpdates::new();
    let mut retry_pending_flush = false;
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

                match collect_stream(
                    &app_handle,
                    websocket,
                    &mut previous_connections,
                    &mut batch_buffer,
                    &mut pending_domain_stats,
                    &mut pending_close_updates,
                    &mut retry_pending_flush,
                    &mut shutdown_rx,
                )
                .await
                {
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

    if current_shutdown(&shutdown_rx).should_close_active() {
        collect_active_connection_closes(&mut previous_connections, &mut pending_close_updates);
    }

    if let Err(error) = flush_pending_state(
        &app_handle,
        &mut batch_buffer,
        &mut pending_domain_stats,
        &mut pending_close_updates,
        &mut retry_pending_flush,
    )
    .await
    {
        warn!("collector 停止前 flush 失败: {error}");
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

async fn collect_stream<R: Runtime>(
    app_handle: &AppHandle<R>,
    websocket: WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>,
    previous_connections: &mut HashMap<String, ConnectionRecord>,
    batch_buffer: &mut BatchBuffer,
    pending_domain_stats: &mut PendingDomainStats,
    pending_close_updates: &mut PendingCloseUpdates,
    retry_pending_flush: &mut bool,
    shutdown_rx: &mut watch::Receiver<CollectorShutdown>,
) -> ConnectionLoopControl {
    let (_, mut read) = websocket.split();
    let mut flush_interval = time::interval(FLUSH_CHECK_INTERVAL);
    flush_interval.set_missed_tick_behavior(MissedTickBehavior::Delay);

    loop {
        let message = tokio::select! {
            changed = shutdown_rx.changed() => {
                if changed.is_err() || should_stop(shutdown_rx) {
                    info!("collector 收到停止信号");
                    return ConnectionLoopControl::Stop;
                }
                continue;
            }
            _ = flush_interval.tick() => {
                if should_flush_pending(batch_buffer, pending_close_updates, *retry_pending_flush) {
                    if let Err(error) = flush_pending_state(
                        app_handle,
                        batch_buffer,
                        pending_domain_stats,
                        pending_close_updates,
                        retry_pending_flush,
                    )
                    .await
                    {
                        warn!("collector 定时 flush 失败: {error}");
                    }
                }
                continue;
            }
            message = read.next() => message,
        };

        match message {
            Some(Ok(Message::Text(text))) => {
                if let Err(error) = apply_snapshot(
                    app_handle,
                    text.as_ref(),
                    previous_connections,
                    batch_buffer,
                    pending_domain_stats,
                    pending_close_updates,
                    retry_pending_flush,
                )
                .await
                {
                    warn!("collector 处理连接快照失败: {error}");
                }
            }
            Some(Ok(Message::Close(frame))) => {
                info!(?frame, "collector WebSocket 已关闭");
                app_handle.state::<RealtimeStore>().clear_active().await;
                if let Err(error) = flush_pending_state(
                    app_handle,
                    batch_buffer,
                    pending_domain_stats,
                    pending_close_updates,
                    retry_pending_flush,
                )
                .await
                {
                    warn!("collector 重连前 flush 失败: {error}");
                }
                return ConnectionLoopControl::Reconnect;
            }
            Some(Ok(_)) => {}
            Some(Err(error)) => {
                warn!("collector WebSocket 读取失败: {error}");
                app_handle.state::<RealtimeStore>().clear_active().await;
                if let Err(error) = flush_pending_state(
                    app_handle,
                    batch_buffer,
                    pending_domain_stats,
                    pending_close_updates,
                    retry_pending_flush,
                )
                .await
                {
                    warn!("collector 读取失败后 flush 失败: {error}");
                }
                return ConnectionLoopControl::Reconnect;
            }
            None => {
                warn!("collector WebSocket 已断开");
                app_handle.state::<RealtimeStore>().clear_active().await;
                if let Err(error) = flush_pending_state(
                    app_handle,
                    batch_buffer,
                    pending_domain_stats,
                    pending_close_updates,
                    retry_pending_flush,
                )
                .await
                {
                    warn!("collector 断开后 flush 失败: {error}");
                }
                return ConnectionLoopControl::Reconnect;
            }
        }
    }
}

async fn apply_snapshot<R: Runtime>(
    app_handle: &AppHandle<R>,
    payload: &str,
    previous_connections: &mut HashMap<String, ConnectionRecord>,
    batch_buffer: &mut BatchBuffer,
    pending_domain_stats: &mut PendingDomainStats,
    pending_close_updates: &mut PendingCloseUpdates,
    retry_pending_flush: &mut bool,
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

    app_handle
        .state::<RealtimeStore>()
        .update_snapshot(current_connections.values().cloned().collect())
        .await;

    let SnapshotDiff {
        opened,
        updated,
        closed,
    } = diff;

    for record in opened {
        enqueue_record(
            app_handle,
            batch_buffer,
            pending_domain_stats,
            pending_close_updates,
            retry_pending_flush,
            record.clone(),
            build_domain_stats_update(&record, None),
        )
        .await;
    }

    for updated_record in updated {
        enqueue_record(
            app_handle,
            batch_buffer,
            pending_domain_stats,
            pending_close_updates,
            retry_pending_flush,
            updated_record.current.clone(),
            build_domain_stats_update(&updated_record.current, Some(&updated_record.previous)),
        )
        .await;
    }

    if !closed.is_empty() {
        let requires_buffer_flush = closed
            .iter()
            .any(|record| batch_buffer.contains_connection(&record.id));

        if requires_buffer_flush {
            if let Err(error) = flush_pending_state(
                app_handle,
                batch_buffer,
                pending_domain_stats,
                pending_close_updates,
                retry_pending_flush,
            )
            .await
            {
                warn!("collector 关闭连接前 flush 失败: {error}");
            }
        }

        if *retry_pending_flush {
            restore_close_updates(pending_close_updates, closed);
        } else if let Err(error) = apply_close_updates(app_handle, &closed).await {
            restore_close_updates(pending_close_updates, closed);
            *retry_pending_flush = true;
            warn!("collector 更新连接关闭时间失败: {error}");
        }
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
                diff.updated.push(UpdatedConnectionRecord {
                    current: current_record.clone(),
                    previous: previous_record.clone(),
                })
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
            id = %record.current.id,
            host = %record.current.host,
            upload = record.current.upload,
            download = record.current.download,
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

async fn wait_for_retry_or_stop(
    shutdown_rx: &mut watch::Receiver<CollectorShutdown>,
    delay: Duration,
) -> bool {
    tokio::select! {
        changed = shutdown_rx.changed() => changed.is_err() || should_stop(shutdown_rx),
        _ = tokio::time::sleep(delay) => false,
    }
}

fn should_stop(shutdown_rx: &watch::Receiver<CollectorShutdown>) -> bool {
    current_shutdown(shutdown_rx).should_stop()
}

fn current_shutdown(shutdown_rx: &watch::Receiver<CollectorShutdown>) -> CollectorShutdown {
    *shutdown_rx.borrow()
}

fn should_flush_pending(
    batch_buffer: &BatchBuffer,
    pending_close_updates: &PendingCloseUpdates,
    retry_pending_flush: bool,
) -> bool {
    retry_pending_flush
        || batch_buffer.should_flush()
        || (!pending_close_updates.is_empty() && batch_buffer.flush_due())
}

fn next_retry_delay(current_delay: Duration) -> Duration {
    (current_delay * 2).min(MAX_RETRY_DELAY)
}

async fn load_open_connections_from_db<R: Runtime>(
    app_handle: &AppHandle<R>,
) -> Result<HashMap<String, ConnectionRecord>, DbError> {
    let db = db::get_db_pool(app_handle).await?;
    repo_connection::list_open_connections(&db).await
}

fn collect_active_connection_closes(
    previous_connections: &mut HashMap<String, ConnectionRecord>,
    pending_close_updates: &mut PendingCloseUpdates,
) {
    let close_time = Utc::now().to_rfc3339();

    for (connection_id, record) in previous_connections.drain() {
        pending_close_updates.insert(
            connection_id.clone(),
            ClosedConnectionRecord {
                id: connection_id,
                host: record.host,
                close_time: close_time.clone(),
            },
        );
    }
}

async fn enqueue_record<R: Runtime>(
    app_handle: &AppHandle<R>,
    batch_buffer: &mut BatchBuffer,
    pending_domain_stats: &mut PendingDomainStats,
    pending_close_updates: &mut PendingCloseUpdates,
    retry_pending_flush: &mut bool,
    record: ConnectionRecord,
    domain_update: Option<DomainStatsUpdate>,
) {
    if let Some(update) = domain_update {
        merge_domain_update(pending_domain_stats, update);
    }

    if let Some(records) = batch_buffer.push(record) {
        if let Err(error) = persist_drained_records(
            app_handle,
            batch_buffer,
            records,
            pending_domain_stats,
            pending_close_updates,
            retry_pending_flush,
        )
        .await
        {
            warn!("collector 容量 flush 失败: {error}");
        }
    }
}

async fn flush_pending_state<R: Runtime>(
    app_handle: &AppHandle<R>,
    batch_buffer: &mut BatchBuffer,
    pending_domain_stats: &mut PendingDomainStats,
    pending_close_updates: &mut PendingCloseUpdates,
    retry_pending_flush: &mut bool,
) -> Result<(), DbError> {
    let records = batch_buffer.flush();
    persist_drained_records(
        app_handle,
        batch_buffer,
        records,
        pending_domain_stats,
        pending_close_updates,
        retry_pending_flush,
    )
    .await
}

async fn persist_drained_records<R: Runtime>(
    app_handle: &AppHandle<R>,
    batch_buffer: &mut BatchBuffer,
    records: Vec<ConnectionRecord>,
    pending_domain_stats: &mut PendingDomainStats,
    pending_close_updates: &mut PendingCloseUpdates,
    retry_pending_flush: &mut bool,
) -> Result<(), DbError> {
    if records.is_empty() && pending_close_updates.is_empty() {
        *retry_pending_flush = false;
        return Ok(());
    }

    let domain_updates = drain_domain_updates(pending_domain_stats);
    let close_updates = drain_close_updates(pending_close_updates);

    let db = match db::get_db_pool(app_handle).await {
        Ok(db) => db,
        Err(error) => {
            batch_buffer.restore(records);
            restore_domain_updates(pending_domain_stats, domain_updates);
            restore_close_updates(pending_close_updates, close_updates);
            *retry_pending_flush = true;
            return Err(error);
        }
    };

    if let Err(error) =
        repo_connection::persist_connection_batch(&db, &records, &domain_updates).await
    {
        batch_buffer.restore(records);
        restore_domain_updates(pending_domain_stats, domain_updates);
        restore_close_updates(pending_close_updates, close_updates);
        *retry_pending_flush = true;
        return Err(error);
    }

    if let Err(error) = apply_close_updates_with_db(&db, &close_updates).await {
        restore_close_updates(pending_close_updates, close_updates);
        *retry_pending_flush = true;
        return Err(error);
    }

    *retry_pending_flush = false;
    Ok(())
}

async fn apply_close_updates<R: Runtime>(
    app_handle: &AppHandle<R>,
    close_updates: &[ClosedConnectionRecord],
) -> Result<(), DbError> {
    if close_updates.is_empty() {
        return Ok(());
    }

    let db = db::get_db_pool(app_handle).await?;
    apply_close_updates_with_db(&db, close_updates).await
}

async fn apply_close_updates_with_db(
    db: &tauri_plugin_sql::DbPool,
    close_updates: &[ClosedConnectionRecord],
) -> Result<(), DbError> {
    for close_update in close_updates {
        repo_connection::update_close_time(db, &close_update.id, &close_update.close_time).await?;
    }

    Ok(())
}

fn build_domain_stats_update(
    current_record: &ConnectionRecord,
    previous_record: Option<&ConnectionRecord>,
) -> Option<DomainStatsUpdate> {
    let domain = current_record.host.trim();
    if domain.is_empty() {
        return None;
    }

    let (hit_count, upload, download) = match previous_record {
        Some(previous_record) => (
            0,
            counter_delta(current_record.upload, previous_record.upload),
            counter_delta(current_record.download, previous_record.download),
        ),
        None => (
            1,
            current_record.upload.max(0),
            current_record.download.max(0),
        ),
    };

    if hit_count == 0 && upload == 0 && download == 0 {
        return None;
    }

    Some(DomainStatsUpdate {
        domain: domain.to_string(),
        day: record_day(&current_record.start_time),
        hit_count,
        upload,
        download,
    })
}

fn record_day(start_time: &str) -> String {
    if let Ok(parsed) = DateTime::parse_from_rfc3339(start_time) {
        return parsed.date_naive().to_string();
    }

    if let Some(day) = start_time.trim().get(..10) {
        return day.to_string();
    }

    Utc::now().date_naive().to_string()
}

fn counter_delta(current_value: i64, previous_value: i64) -> i64 {
    if current_value >= previous_value {
        current_value - previous_value
    } else {
        current_value.max(0)
    }
}

fn merge_domain_update(pending_domain_stats: &mut PendingDomainStats, update: DomainStatsUpdate) {
    let key = (update.domain.clone(), update.day.clone());

    if let Some(existing) = pending_domain_stats.get_mut(&key) {
        existing.hit_count += update.hit_count;
        existing.upload += update.upload;
        existing.download += update.download;
    } else {
        pending_domain_stats.insert(key, update);
    }
}

fn drain_domain_updates(pending_domain_stats: &mut PendingDomainStats) -> Vec<DomainStatsUpdate> {
    pending_domain_stats
        .drain()
        .map(|(_, update)| update)
        .collect()
}

fn restore_domain_updates(
    pending_domain_stats: &mut PendingDomainStats,
    updates: Vec<DomainStatsUpdate>,
) {
    for update in updates {
        merge_domain_update(pending_domain_stats, update);
    }
}

fn drain_close_updates(
    pending_close_updates: &mut PendingCloseUpdates,
) -> Vec<ClosedConnectionRecord> {
    pending_close_updates
        .drain()
        .map(|(_, update)| update)
        .collect()
}

fn restore_close_updates(
    pending_close_updates: &mut PendingCloseUpdates,
    updates: Vec<ClosedConnectionRecord>,
) {
    for update in updates {
        pending_close_updates.insert(update.id.clone(), update);
    }
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
        assert_eq!(diff.updated[0].current.id, "kept");
        assert_eq!(diff.closed[0].id, "closed");
    }

    #[test]
    fn build_domain_stats_update_uses_deltas_for_updated_records() {
        let previous_record = sample_record("same", "example.com", 4, 8);
        let current_record = sample_record("same", "example.com", 10, 12);

        let update = build_domain_stats_update(&current_record, Some(&previous_record));

        assert!(update.is_some());
        let Some(update) = update else {
            panic!("updated connection should emit a domain stats delta");
        };

        assert_eq!(update.hit_count, 0);
        assert_eq!(update.upload, 6);
        assert_eq!(update.download, 4);
        assert_eq!(update.day, "2026-03-30");
    }

    #[test]
    fn collect_active_connection_closes_drains_previous_connections() {
        let mut previous_connections = HashMap::from([(
            "still-open".to_string(),
            sample_record("still-open", "example.com", 3, 7),
        )]);
        let mut pending_close_updates = PendingCloseUpdates::new();

        collect_active_connection_closes(&mut previous_connections, &mut pending_close_updates);

        assert!(previous_connections.is_empty());
        assert_eq!(pending_close_updates.len(), 1);
        let close_update = pending_close_updates.get("still-open");
        assert!(close_update.is_some());
        let Some(close_update) = close_update else {
            panic!("active connection should be converted into a close update");
        };

        assert_eq!(close_update.host, "example.com");
        assert!(!close_update.close_time.is_empty());
    }
}
