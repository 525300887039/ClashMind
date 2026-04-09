use std::{
    collections::{BTreeSet, HashMap},
    time::Duration,
};

use chrono::{DateTime, Utc};
use futures_util::StreamExt;
use reqwest::Url;
use serde::{Deserialize, Deserializer, Serialize};
use tauri::{AppHandle, Manager, Runtime};
use tokio::{
    sync::watch,
    time::{self, MissedTickBehavior},
};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{
        protocol::{frame::coding::CloseCode, CloseFrame},
        Message,
    },
    MaybeTlsStream, WebSocketStream,
};
use tracing::{info, warn};

use crate::{
    collector::{
        buffer::{BatchBuffer, DEFAULT_BATCH_CAPACITY, DEFAULT_FLUSH_INTERVAL},
        RealtimeStore,
    },
    db::{
        self,
        repo_connection::{self, BatchPersistPayload, RuleStatsUpdate},
        repo_domain::DomainStatsUpdate,
        repo_error_log::{self, ErrorLogInsert},
        repo_geoip::IpTrafficStatsUpdate,
        repo_traffic::TrafficSampleInsert,
        DbError,
    },
    utils::time as time_utils,
};

use super::{CollectorError, CollectorShutdown};

const INITIAL_RETRY_DELAY: Duration = Duration::from_secs(1);
const MAX_RETRY_DELAY: Duration = Duration::from_secs(30);
const FLUSH_CHECK_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    #[serde(skip_serializing, skip_deserializing, default)]
    pub last_observed_at: Option<String>,
}

impl PartialEq for ConnectionRecord {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
            && self.host == other.host
            && self.dst_ip == other.dst_ip
            && self.dst_port == other.dst_port
            && self.src_ip == other.src_ip
            && self.src_port == other.src_port
            && self.network == other.network
            && self.conn_type == other.conn_type
            && self.rule == other.rule
            && self.rule_payload == other.rule_payload
            && self.proxy_chain == other.proxy_chain
            && self.upload == other.upload
            && self.download == other.download
            && self.start_time == other.start_time
    }
}

impl Eq for ConnectionRecord {}

#[derive(Debug, Deserialize)]
struct ConnectionsSnapshot {
    #[serde(default, deserialize_with = "deserialize_null_default")]
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
    #[serde(default, deserialize_with = "deserialize_null_default")]
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

fn deserialize_null_default<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de> + Default,
{
    Option::<T>::deserialize(deserializer).map(|value| value.unwrap_or_default())
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
type PendingRuleStats = HashMap<(String, String), RuleStatsUpdate>;
type PendingIpTrafficStats = HashMap<(String, String), IpTrafficStatsUpdate>;
type PendingObservationRecords = HashMap<String, ConnectionRecord>;
type PendingTrafficSamples = Vec<TrafficSampleInsert>;
type PendingCloseUpdates = HashMap<String, ClosedConnectionRecord>;

fn merge_into<K, V>(map: &mut HashMap<K, V>, key: K, value: V, merge_fn: impl FnOnce(&mut V, V))
where
    K: Eq + std::hash::Hash,
{
    match map.entry(key) {
        std::collections::hash_map::Entry::Occupied(mut entry) => {
            merge_fn(entry.get_mut(), value);
        }
        std::collections::hash_map::Entry::Vacant(entry) => {
            entry.insert(value);
        }
    }
}

fn merge_domain_stats(existing: &mut DomainStatsUpdate, incoming: DomainStatsUpdate) {
    existing.hit_count += incoming.hit_count;
    existing.upload += incoming.upload;
    existing.download += incoming.download;
}

fn merge_rule_stats(existing: &mut RuleStatsUpdate, incoming: RuleStatsUpdate) {
    existing.hit_count += incoming.hit_count;
    existing.upload += incoming.upload;
    existing.download += incoming.download;
}

fn merge_ip_traffic_stats(existing: &mut IpTrafficStatsUpdate, incoming: IpTrafficStatsUpdate) {
    existing.upload += incoming.upload;
    existing.download += incoming.download;
}

struct PendingFlushState {
    batch_buffer: BatchBuffer,
    domain_stats: PendingDomainStats,
    rule_stats: PendingRuleStats,
    ip_traffic_stats: PendingIpTrafficStats,
    observation_records: PendingObservationRecords,
    traffic_samples: PendingTrafficSamples,
    close_updates: PendingCloseUpdates,
    retry_flush: bool,
}

struct DrainedState {
    domain_updates: Vec<DomainStatsUpdate>,
    rule_updates: Vec<RuleStatsUpdate>,
    ip_traffic_updates: Vec<IpTrafficStatsUpdate>,
    observation_records: Vec<ConnectionRecord>,
    traffic_samples: Vec<TrafficSampleInsert>,
    close_updates: Vec<ClosedConnectionRecord>,
}

impl PendingFlushState {
    fn new() -> Self {
        Self {
            batch_buffer: BatchBuffer::new(DEFAULT_BATCH_CAPACITY, DEFAULT_FLUSH_INTERVAL),
            domain_stats: PendingDomainStats::new(),
            rule_stats: PendingRuleStats::new(),
            ip_traffic_stats: PendingIpTrafficStats::new(),
            observation_records: PendingObservationRecords::new(),
            traffic_samples: PendingTrafficSamples::new(),
            close_updates: PendingCloseUpdates::new(),
            retry_flush: false,
        }
    }

    fn should_flush(&self) -> bool {
        self.retry_flush
            || self.batch_buffer.should_flush()
            || (!self.observation_records.is_empty() && self.batch_buffer.flush_due())
            || (!self.close_updates.is_empty() && self.batch_buffer.flush_due())
    }

    fn is_all_empty(&self) -> bool {
        self.domain_stats.is_empty()
            && self.rule_stats.is_empty()
            && self.ip_traffic_stats.is_empty()
            && self.observation_records.is_empty()
            && self.traffic_samples.is_empty()
            && self.close_updates.is_empty()
    }

    fn enqueue_updates(&mut self, updates: ObservationUpdates, record_id: &str) {
        self.observation_records.remove(record_id);

        for update in updates.domain_updates {
            let key = (update.domain.clone(), update.day.clone());
            merge_into(&mut self.domain_stats, key, update, merge_domain_stats);
        }
        for update in updates.rule_updates {
            let key = (update.rule.clone(), update.day.clone());
            merge_into(&mut self.rule_stats, key, update, merge_rule_stats);
        }
        for update in updates.ip_traffic_updates {
            let key = (update.dst_ip.clone(), update.day.clone());
            merge_into(
                &mut self.ip_traffic_stats,
                key,
                update,
                merge_ip_traffic_stats,
            );
        }
        self.traffic_samples.extend(updates.traffic_samples);
    }

    fn drain_all(&mut self) -> DrainedState {
        DrainedState {
            domain_updates: self.domain_stats.drain().map(|(_, v)| v).collect(),
            rule_updates: self.rule_stats.drain().map(|(_, v)| v).collect(),
            ip_traffic_updates: self.ip_traffic_stats.drain().map(|(_, v)| v).collect(),
            observation_records: self.observation_records.drain().map(|(_, v)| v).collect(),
            traffic_samples: std::mem::take(&mut self.traffic_samples),
            close_updates: self.close_updates.drain().map(|(_, v)| v).collect(),
        }
    }

    fn restore_all(&mut self, drained: DrainedState) {
        for update in drained.domain_updates {
            let key = (update.domain.clone(), update.day.clone());
            merge_into(&mut self.domain_stats, key, update, merge_domain_stats);
        }
        for update in drained.rule_updates {
            let key = (update.rule.clone(), update.day.clone());
            merge_into(&mut self.rule_stats, key, update, merge_rule_stats);
        }
        for update in drained.ip_traffic_updates {
            let key = (update.dst_ip.clone(), update.day.clone());
            merge_into(
                &mut self.ip_traffic_stats,
                key,
                update,
                merge_ip_traffic_stats,
            );
        }
        for record in drained.observation_records {
            self.observation_records.insert(record.id.clone(), record);
        }
        self.traffic_samples.extend(drained.traffic_samples);
        for update in drained.close_updates {
            self.close_updates.insert(update.id.clone(), update);
        }
    }
}

#[derive(Debug, Default)]
struct ObservationUpdates {
    domain_updates: Vec<DomainStatsUpdate>,
    rule_updates: Vec<RuleStatsUpdate>,
    ip_traffic_updates: Vec<IpTrafficStatsUpdate>,
    traffic_samples: Vec<TrafficSampleInsert>,
}

impl RawConnection {
    fn into_record(self, observed_at: &str) -> Result<ConnectionRecord, CollectorError> {
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
            last_observed_at: Some(observed_at.to_string()),
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
    let mut pending = PendingFlushState::new();
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
                    &mut pending,
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
        collect_active_connection_closes(&mut previous_connections, &mut pending.close_updates);
    }

    if let Err(error) = flush_pending_state(&app_handle, &mut pending).await {
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
    state: &mut PendingFlushState,
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
                if state.should_flush() {
                    if let Err(error) = flush_pending_state(app_handle, state).await {
                        warn!("collector 定时 flush 失败: {error}");
                    }
                }
                continue;
            }
            message = read.next() => message,
        };

        match message {
            Some(Ok(Message::Text(text))) => {
                if let Err(error) =
                    apply_snapshot(app_handle, text.as_ref(), previous_connections, state).await
                {
                    warn!("collector 处理连接快照失败: {error}");
                }
            }
            Some(Ok(Message::Close(frame))) => {
                info!(?frame, "collector WebSocket 已关闭");
                return handle_disconnect(
                    app_handle,
                    previous_connections,
                    state,
                    "重连前",
                    disconnect_message_from_close_frame(frame.as_ref()),
                )
                .await;
            }
            Some(Ok(_)) => {}
            Some(Err(error)) => {
                warn!("collector WebSocket 读取失败: {error}");
                return handle_disconnect(
                    app_handle,
                    previous_connections,
                    state,
                    "读取失败后",
                    Some(format!("collector websocket read failed: {error}")),
                )
                .await;
            }
            None => {
                warn!("collector WebSocket 已断开");
                return handle_disconnect(
                    app_handle,
                    previous_connections,
                    state,
                    "断开后",
                    Some("collector websocket disconnected without close frame".to_string()),
                )
                .await;
            }
        }
    }
}

async fn handle_disconnect<R: Runtime>(
    app_handle: &AppHandle<R>,
    previous_connections: &HashMap<String, ConnectionRecord>,
    state: &mut PendingFlushState,
    reason: &str,
    disconnect_message: Option<String>,
) -> ConnectionLoopControl {
    app_handle.state::<RealtimeStore>().clear_active().await;
    if let Err(error) = flush_pending_state(app_handle, state).await {
        warn!("collector {reason} flush 失败: {error}");
    }
    if let Some(message) = disconnect_message.as_deref() {
        record_disconnect_errors(app_handle, previous_connections, message).await;
    }
    ConnectionLoopControl::Reconnect
}

fn disconnect_message_from_close_frame(frame: Option<&CloseFrame>) -> Option<String> {
    match frame {
        Some(frame) if is_expected_close_code(frame.code) => None,
        Some(frame) if !frame.reason.trim().is_empty() => Some(format!(
            "collector websocket closed: code={}, reason={}",
            frame.code, frame.reason
        )),
        Some(frame) => Some(format!("collector websocket closed: code={}", frame.code)),
        None => Some("collector websocket closed without close frame".to_string()),
    }
}

fn classify_error(message: &str) -> &'static str {
    let normalized = message.to_ascii_lowercase();

    if normalized.contains("timeout")
        || normalized.contains("timed out")
        || normalized.contains("deadline exceeded")
        || normalized.contains("i/o timeout")
    {
        "timeout"
    } else if normalized.contains("dns")
        || normalized.contains("resolve")
        || normalized.contains("resolved")
        || normalized.contains("lookup")
        || normalized.contains("no such host")
        || normalized.contains("name or service not known")
    {
        "dns"
    } else if normalized.contains("tls")
        || normalized.contains("ssl")
        || normalized.contains("certificate")
        || normalized.contains("handshake")
        || normalized.contains("x509")
    {
        "tls"
    } else if normalized.contains("refused")
        || normalized.contains("reset")
        || normalized.contains("broken pipe")
        || normalized.contains("network is unreachable")
        || normalized.contains("connection aborted")
        || normalized.contains("unexpected eof")
    {
        "connection"
    } else {
        "other"
    }
}

fn is_expected_close_code(code: CloseCode) -> bool {
    matches!(
        code,
        CloseCode::Normal | CloseCode::Away | CloseCode::Restart
    )
}

fn normalize_optional_text(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn extract_proxy_node(proxy_chain: &str) -> Option<String> {
    serde_json::from_str::<Vec<String>>(proxy_chain)
        .ok()
        .and_then(|entries| {
            entries
                .into_iter()
                .rev()
                .find_map(|entry| normalize_optional_text(&entry))
        })
        .or_else(|| normalize_optional_text(proxy_chain))
}

fn build_disconnect_error_logs(
    previous_connections: &HashMap<String, ConnectionRecord>,
    message: &str,
) -> Vec<ErrorLogInsert> {
    let trimmed = message.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }

    let category = classify_error(trimmed).to_string();
    if previous_connections.is_empty() {
        return vec![ErrorLogInsert {
            category,
            proxy_node: None,
            host: None,
            rule: None,
            message: trimmed.to_string(),
        }];
    }

    let mut metadata = BTreeSet::new();
    for record in previous_connections.values() {
        metadata.insert((
            extract_proxy_node(&record.proxy_chain),
            normalize_optional_text(&record.host),
            normalize_optional_text(&record.rule),
        ));
    }

    metadata
        .into_iter()
        .map(|(proxy_node, host, rule)| ErrorLogInsert {
            category: category.clone(),
            proxy_node,
            host,
            rule,
            message: trimmed.to_string(),
        })
        .collect()
}

async fn record_disconnect_errors<R: Runtime>(
    app_handle: &AppHandle<R>,
    previous_connections: &HashMap<String, ConnectionRecord>,
    message: &str,
) {
    let logs = build_disconnect_error_logs(previous_connections, message);
    if logs.is_empty() {
        return;
    }

    let db = match db::get_db_pool(app_handle).await {
        Ok(db) => db,
        Err(error) => {
            warn!("collector 写入断连错误日志前获取数据库失败: {error}");
            return;
        }
    };

    if let Err(error) = repo_error_log::insert_error_logs_batch(&db, &logs).await {
        warn!("collector 写入断连错误日志失败: {error}");
    }
}

async fn apply_snapshot<R: Runtime>(
    app_handle: &AppHandle<R>,
    payload: &str,
    previous_connections: &mut HashMap<String, ConnectionRecord>,
    state: &mut PendingFlushState,
) -> Result<(), CollectorError> {
    let snapshot: ConnectionsSnapshot = serde_json::from_str(payload)
        .map_err(|error| CollectorError::SnapshotParse(error.to_string()))?;
    let mut current_connections = HashMap::with_capacity(snapshot.connections.len());
    let observed_at = Utc::now().to_rfc3339();

    for raw_connection in snapshot.connections {
        let record = raw_connection.into_record(&observed_at)?;
        current_connections.insert(record.id.clone(), record);
    }

    let diff = diff_snapshots(previous_connections, &current_connections);
    if !diff.is_empty() {
        log_snapshot_diff(&diff, current_connections.len());
    }
    sync_pending_observation_records(
        previous_connections,
        &current_connections,
        &mut state.observation_records,
    );

    let mut closed_ids = Vec::with_capacity(diff.closed.len());
    closed_ids.extend(diff.closed.iter().map(|record| record.id.clone()));

    app_handle
        .state::<RealtimeStore>()
        .apply_diff(
            current_connections.clone(),
            &closed_ids,
            diff.opened.len(),
            diff.updated.len(),
        )
        .await;

    let SnapshotDiff {
        opened,
        updated,
        closed,
    } = diff;

    for record in opened {
        let updates = build_observation_updates(&record, None, &observed_at);
        enqueue_record(app_handle, state, record, updates).await;
    }

    for updated_record in updated {
        let updates = build_observation_updates(
            &updated_record.current,
            Some(&updated_record.previous),
            &observed_at,
        );
        enqueue_record(app_handle, state, updated_record.current, updates).await;
    }

    if !closed.is_empty() {
        let requires_buffer_flush = closed
            .iter()
            .any(|record| state.batch_buffer.contains_connection(&record.id));

        if requires_buffer_flush {
            if let Err(error) = flush_pending_state(app_handle, state).await {
                warn!("collector 关闭连接前 flush 失败: {error}");
            }
        }

        if state.retry_flush {
            restore_close_updates(&mut state.close_updates, closed);
        } else if let Err(error) = apply_close_updates(app_handle, &closed).await {
            restore_close_updates(&mut state.close_updates, closed);
            state.retry_flush = true;
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

    let close_time = Utc::now().to_rfc3339();
    for (connection_id, previous_record) in previous_connections {
        if !current_connections.contains_key(connection_id) {
            diff.closed.push(ClosedConnectionRecord {
                id: connection_id.clone(),
                host: previous_record.host.clone(),
                close_time: close_time.clone(),
            });
        }
    }

    diff
}

fn sync_pending_observation_records(
    previous_connections: &HashMap<String, ConnectionRecord>,
    current_connections: &HashMap<String, ConnectionRecord>,
    pending_observation_records: &mut PendingObservationRecords,
) {
    for (connection_id, current_record) in current_connections {
        match previous_connections.get(connection_id) {
            Some(previous_record) if previous_record == current_record => {
                pending_observation_records.insert(connection_id.clone(), current_record.clone());
            }
            _ => {
                pending_observation_records.remove(connection_id);
            }
        }
    }

    for connection_id in previous_connections.keys() {
        if !current_connections.contains_key(connection_id) {
            pending_observation_records.remove(connection_id);
        }
    }
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
    state: &mut PendingFlushState,
    record: ConnectionRecord,
    updates: ObservationUpdates,
) {
    state.enqueue_updates(updates, &record.id);

    if let Some(records) = state.batch_buffer.push(record) {
        if let Err(error) = persist_drained_records(app_handle, state, records).await {
            warn!("collector 容量 flush 失败: {error}");
        }
    }
}

async fn flush_pending_state<R: Runtime>(
    app_handle: &AppHandle<R>,
    state: &mut PendingFlushState,
) -> Result<(), DbError> {
    let records = state.batch_buffer.flush();
    persist_drained_records(app_handle, state, records).await
}

async fn persist_drained_records<R: Runtime>(
    app_handle: &AppHandle<R>,
    state: &mut PendingFlushState,
    records: Vec<ConnectionRecord>,
) -> Result<(), DbError> {
    if records.is_empty() && state.is_all_empty() {
        state.retry_flush = false;
        return Ok(());
    }

    let drained = state.drain_all();

    let db = match db::get_db_pool(app_handle).await {
        Ok(db) => db,
        Err(error) => {
            state.batch_buffer.restore(records);
            state.restore_all(drained);
            state.retry_flush = true;
            return Err(error);
        }
    };

    let payload = BatchPersistPayload {
        records: &records,
        observation_records: &drained.observation_records,
        domain_updates: &drained.domain_updates,
        rule_updates: &drained.rule_updates,
        ip_traffic_updates: &drained.ip_traffic_updates,
        traffic_samples: &drained.traffic_samples,
    };

    if let Err(error) = repo_connection::persist_connection_batch(&db, &payload).await {
        state.batch_buffer.restore(records);
        state.restore_all(drained);
        state.retry_flush = true;
        return Err(error);
    }

    if let Err(error) = apply_close_updates_with_db(&db, &drained.close_updates).await {
        restore_close_updates(&mut state.close_updates, drained.close_updates);
        state.retry_flush = true;
        return Err(error);
    }

    state.retry_flush = false;
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

#[derive(Debug, Clone)]
struct WeightedBucket<T> {
    key: T,
    weight_ms: i64,
}

fn build_observation_updates(
    current_record: &ConnectionRecord,
    previous_record: Option<&ConnectionRecord>,
    observed_at: &str,
) -> ObservationUpdates {
    let domain = current_record.host.trim();
    let (upload, download) = match previous_record {
        Some(previous_record) => (
            counter_delta(current_record.upload, previous_record.upload),
            counter_delta(current_record.download, previous_record.download),
        ),
        None => (current_record.upload.max(0), current_record.download.max(0)),
    };
    let is_new_connection = previous_record.is_none();
    let domain_updates = if domain.is_empty() {
        Vec::new()
    } else {
        build_domain_stats_updates(
            domain,
            upload,
            download,
            is_new_connection,
            current_record,
            previous_record,
            observed_at,
        )
    };
    let rule_updates = build_rule_stats_updates(
        &normalize_rule_name(&current_record.rule),
        upload,
        download,
        is_new_connection,
        current_record,
        previous_record,
        observed_at,
    );
    let traffic_samples = build_traffic_samples(
        upload,
        download,
        is_new_connection,
        current_record,
        previous_record,
        observed_at,
    );
    let ip_traffic_updates = match current_record.dst_ip.as_deref().map(str::trim) {
        Some(dst_ip) if !dst_ip.is_empty() => build_ip_traffic_stats_updates(
            dst_ip,
            upload,
            download,
            is_new_connection,
            current_record,
            previous_record,
            observed_at,
        ),
        _ => Vec::new(),
    };

    ObservationUpdates {
        domain_updates,
        rule_updates,
        ip_traffic_updates,
        traffic_samples,
    }
}

fn distribute_and_build<T>(
    upload: i64,
    download: i64,
    is_new_connection: bool,
    buckets: Vec<WeightedBucket<String>>,
    count_new: bool,
    build_item: impl Fn(String, i64, i64, i64) -> T,
) -> Vec<T> {
    let uploads = distribute_total(upload, &buckets);
    let downloads = distribute_total(download, &buckets);
    let mut items = Vec::with_capacity(buckets.len());

    for (index, bucket) in buckets.into_iter().enumerate() {
        let count = if count_new && is_new_connection && index == 0 {
            1
        } else {
            0
        };
        let up = uploads.get(index).copied().unwrap_or(0);
        let down = downloads.get(index).copied().unwrap_or(0);

        if count == 0 && up == 0 && down == 0 {
            continue;
        }

        items.push(build_item(bucket.key, up, down, count));
    }

    items
}

fn build_domain_stats_updates(
    domain: &str,
    upload: i64,
    download: i64,
    is_new_connection: bool,
    current_record: &ConnectionRecord,
    previous_record: Option<&ConnectionRecord>,
    observed_at: &str,
) -> Vec<DomainStatsUpdate> {
    if !is_new_connection && upload == 0 && download == 0 {
        return Vec::new();
    }

    let (start, end) = observation_window(current_record, previous_record, observed_at);
    distribute_and_build(
        upload,
        download,
        is_new_connection,
        build_day_buckets(start, end),
        true,
        |day, up, down, count| DomainStatsUpdate {
            domain: domain.to_string(),
            day,
            hit_count: count,
            upload: up,
            download: down,
        },
    )
}

fn build_rule_stats_updates(
    rule: &str,
    upload: i64,
    download: i64,
    is_new_connection: bool,
    current_record: &ConnectionRecord,
    previous_record: Option<&ConnectionRecord>,
    observed_at: &str,
) -> Vec<RuleStatsUpdate> {
    if !is_new_connection && upload == 0 && download == 0 {
        return Vec::new();
    }

    let (start, end) = observation_window(current_record, previous_record, observed_at);
    distribute_and_build(
        upload,
        download,
        is_new_connection,
        build_day_buckets(start, end),
        true,
        |day, up, down, count| RuleStatsUpdate {
            rule: rule.to_string(),
            day,
            hit_count: count,
            upload: up,
            download: down,
        },
    )
}

fn build_ip_traffic_stats_updates(
    dst_ip: &str,
    upload: i64,
    download: i64,
    is_new_connection: bool,
    current_record: &ConnectionRecord,
    previous_record: Option<&ConnectionRecord>,
    observed_at: &str,
) -> Vec<IpTrafficStatsUpdate> {
    if upload == 0 && download == 0 {
        return Vec::new();
    }

    let (start, end) = observation_window(current_record, previous_record, observed_at);
    distribute_and_build(
        upload,
        download,
        is_new_connection,
        build_day_buckets(start, end),
        false,
        |day, up, down, _count| IpTrafficStatsUpdate {
            dst_ip: dst_ip.to_string(),
            day,
            upload: up,
            download: down,
        },
    )
}

fn build_traffic_samples(
    upload: i64,
    download: i64,
    is_new_connection: bool,
    current_record: &ConnectionRecord,
    previous_record: Option<&ConnectionRecord>,
    observed_at: &str,
) -> Vec<TrafficSampleInsert> {
    if !is_new_connection && upload == 0 && download == 0 {
        return Vec::new();
    }

    let (start, end) = observation_window(current_record, previous_record, observed_at);
    distribute_and_build(
        upload,
        download,
        is_new_connection,
        build_hour_buckets(start, end),
        true,
        |ts, up, down, count| TrafficSampleInsert {
            ts,
            upload: up,
            download: down,
            conn_count: count,
        },
    )
}

fn observation_window(
    current_record: &ConnectionRecord,
    previous_record: Option<&ConnectionRecord>,
    observed_at: &str,
) -> (DateTime<Utc>, DateTime<Utc>) {
    let end = time_utils::parse_rfc3339(observed_at).unwrap_or_else(Utc::now);
    let baseline = previous_record
        .and_then(|record| record.last_observed_at.as_deref())
        .or_else(|| previous_record.map(|record| record.start_time.as_str()))
        .unwrap_or(&current_record.start_time);
    let start = time_utils::parse_rfc3339(baseline).unwrap_or(end);

    if start < end {
        (start, end)
    } else {
        (end, end)
    }
}

fn build_hour_buckets(start: DateTime<Utc>, end: DateTime<Utc>) -> Vec<WeightedBucket<String>> {
    build_weighted_buckets(
        start,
        end,
        time_utils::hour_bucket_key,
        time_utils::next_hour_boundary,
    )
}

fn build_day_buckets(start: DateTime<Utc>, end: DateTime<Utc>) -> Vec<WeightedBucket<String>> {
    build_weighted_buckets(
        start,
        end,
        time_utils::day_bucket_key,
        time_utils::next_day_boundary,
    )
}

fn build_weighted_buckets<T, FKey, FNext>(
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    key_fn: FKey,
    next_boundary_fn: FNext,
) -> Vec<WeightedBucket<T>>
where
    FKey: Fn(&DateTime<Utc>) -> T,
    FNext: Fn(&DateTime<Utc>) -> DateTime<Utc>,
{
    if end <= start {
        return vec![WeightedBucket {
            key: key_fn(&end),
            weight_ms: 1,
        }];
    }

    let mut buckets = Vec::new();
    let mut cursor = start;

    while cursor < end {
        let next_boundary = next_boundary_fn(&cursor);
        let segment_end = if next_boundary < end {
            next_boundary
        } else {
            end
        };
        let weight_ms = (segment_end - cursor).num_milliseconds().max(1);

        buckets.push(WeightedBucket {
            key: key_fn(&cursor),
            weight_ms,
        });

        cursor = segment_end;
    }

    if buckets.is_empty() {
        buckets.push(WeightedBucket {
            key: key_fn(&end),
            weight_ms: 1,
        });
    }

    buckets
}

fn distribute_total<T>(total: i64, buckets: &[WeightedBucket<T>]) -> Vec<i64> {
    if buckets.is_empty() {
        return Vec::new();
    }

    if total <= 0 {
        return vec![0; buckets.len()];
    }

    let mut remaining_total = total;
    let mut remaining_weight: i64 = buckets.iter().map(|bucket| bucket.weight_ms.max(1)).sum();
    let mut parts = Vec::with_capacity(buckets.len());

    for (index, bucket) in buckets.iter().enumerate() {
        let weight = bucket.weight_ms.max(1);
        let value = if index + 1 == buckets.len() || remaining_weight <= 0 {
            remaining_total
        } else {
            remaining_total.saturating_mul(weight) / remaining_weight
        };

        parts.push(value);
        remaining_total -= value;
        remaining_weight -= weight;
    }

    parts
}

fn counter_delta(current_value: i64, previous_value: i64) -> i64 {
    if current_value >= previous_value {
        current_value - previous_value
    } else {
        current_value.max(0)
    }
}

fn normalize_rule_name(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        "UNKNOWN".to_string()
    } else {
        trimmed.to_string()
    }
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
            last_observed_at: Some("2026-03-30T10:00:00.000Z".into()),
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
    fn classify_error_covers_common_disconnect_patterns() {
        assert_eq!(classify_error("i/o timeout"), "timeout");
        assert_eq!(classify_error("lookup api.example: no such host"), "dns");
        assert_eq!(classify_error("tls handshake failure"), "tls");
        assert_eq!(classify_error("connection reset by peer"), "connection");
        assert_eq!(classify_error("unexpected websocket close"), "other");
    }

    #[test]
    fn disconnect_message_from_close_frame_skips_expected_close_codes() {
        assert_eq!(
            disconnect_message_from_close_frame(Some(&CloseFrame {
                code: CloseCode::Normal,
                reason: "normal shutdown".into(),
            })),
            None
        );
        assert_eq!(
            disconnect_message_from_close_frame(Some(&CloseFrame {
                code: CloseCode::Away,
                reason: "server going away".into(),
            })),
            None
        );
        assert_eq!(
            disconnect_message_from_close_frame(Some(&CloseFrame {
                code: CloseCode::Restart,
                reason: "restart".into(),
            })),
            None
        );
    }

    #[test]
    fn build_disconnect_error_logs_uses_active_connection_metadata() {
        let mut proxy_record = sample_record("proxy", "api.example", 1, 1);
        proxy_record.rule = "MATCH".into();
        proxy_record.proxy_chain = "[\"Selector\", \"Proxy-A\"]".into();

        let mut direct_record = sample_record("direct", "dns.example", 1, 1);
        direct_record.rule = "RULE-SET".into();
        direct_record.proxy_chain = "[\"DIRECT\"]".into();

        let previous_connections = HashMap::from([
            ("proxy".to_string(), proxy_record),
            ("direct".to_string(), direct_record),
        ]);

        let logs =
            build_disconnect_error_logs(&previous_connections, "lookup api.example: no such host");

        assert_eq!(logs.len(), 2);
        assert_eq!(logs[0].category, "dns");
        assert_eq!(logs[0].message, "lookup api.example: no such host");
        assert_eq!(logs[0].proxy_node.as_deref(), Some("DIRECT"));
        assert_eq!(logs[0].host.as_deref(), Some("dns.example"));
        assert_eq!(logs[0].rule.as_deref(), Some("RULE-SET"));
        assert_eq!(logs[1].proxy_node.as_deref(), Some("Proxy-A"));
        assert_eq!(logs[1].host.as_deref(), Some("api.example"));
        assert_eq!(logs[1].rule.as_deref(), Some("MATCH"));
    }

    #[test]
    fn connections_snapshot_treats_null_connections_as_empty() {
        let payload = r#"{"downloadTotal":0,"uploadTotal":0,"connections":null}"#;

        let snapshot = serde_json::from_str::<ConnectionsSnapshot>(payload);

        assert!(snapshot.is_ok());
        let Ok(snapshot) = snapshot else {
            panic!("snapshot with null connections should deserialize");
        };
        assert!(snapshot.connections.is_empty());
    }

    #[test]
    fn raw_connection_treats_null_chains_as_empty() {
        let payload = r#"{
            "id":"conn-1",
            "metadata":{"host":"example.com"},
            "upload":0,
            "download":0,
            "start":"2026-03-30T10:00:00Z",
            "chains":null,
            "rule":"MATCH",
            "rulePayload":null
        }"#;

        let raw = serde_json::from_str::<RawConnection>(payload);

        assert!(raw.is_ok());
        let Ok(raw) = raw else {
            panic!("raw connection with null chains should deserialize");
        };
        assert!(raw.chains.is_empty());
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
    fn sync_pending_observation_records_refreshes_unchanged_connections() {
        let previous_record = sample_record("same", "example.com", 4, 8);
        let mut current_record = sample_record("same", "example.com", 4, 8);
        current_record.last_observed_at = Some("2026-03-30T10:30:00Z".into());

        let previous_connections = HashMap::from([("same".to_string(), previous_record)]);
        let current_connections = HashMap::from([("same".to_string(), current_record.clone())]);
        let mut pending_observation_records = PendingObservationRecords::new();

        sync_pending_observation_records(
            &previous_connections,
            &current_connections,
            &mut pending_observation_records,
        );

        assert_eq!(pending_observation_records.len(), 1);
        let record = pending_observation_records.get("same");
        assert!(record.is_some());
        let Some(record) = record else {
            panic!("unchanged connection should be queued for observation persistence");
        };
        assert_eq!(
            record.last_observed_at.as_deref(),
            Some("2026-03-30T10:30:00Z")
        );
    }

    #[test]
    fn build_observation_updates_use_deltas_for_updated_records() {
        let previous_record = sample_record("same", "example.com", 4, 8);
        let current_record = sample_record("same", "example.com", 10, 12);

        let updates = build_observation_updates(
            &current_record,
            Some(&previous_record),
            "2026-03-30T10:05:00Z",
        );

        assert_eq!(updates.domain_updates.len(), 1);
        assert_eq!(updates.domain_updates[0].hit_count, 0);
        assert_eq!(updates.domain_updates[0].upload, 6);
        assert_eq!(updates.domain_updates[0].download, 4);
        assert_eq!(updates.domain_updates[0].day, "2026-03-30");
        assert_eq!(updates.rule_updates.len(), 1);
        assert_eq!(updates.rule_updates[0].rule, "DOMAIN-SUFFIX");
        assert_eq!(updates.rule_updates[0].hit_count, 0);
        assert_eq!(updates.rule_updates[0].upload, 6);
        assert_eq!(updates.rule_updates[0].download, 4);
        assert_eq!(updates.ip_traffic_updates.len(), 1);
        assert_eq!(updates.ip_traffic_updates[0].dst_ip, "1.1.1.1");
        assert_eq!(updates.ip_traffic_updates[0].day, "2026-03-30");
        assert_eq!(updates.ip_traffic_updates[0].upload, 6);
        assert_eq!(updates.ip_traffic_updates[0].download, 4);
        assert_eq!(updates.traffic_samples.len(), 1);
        assert_eq!(updates.traffic_samples[0].ts, "2026-03-30T10:00:00Z");
        assert_eq!(updates.traffic_samples[0].upload, 6);
        assert_eq!(updates.traffic_samples[0].download, 4);
        assert_eq!(updates.traffic_samples[0].conn_count, 0);
    }

    #[test]
    fn build_observation_updates_split_cross_day_recovery_windows() {
        let mut previous_record = sample_record("same", "example.com", 40, 80);
        let current_record = sample_record("same", "example.com", 55, 95);
        previous_record.last_observed_at = Some("2026-03-30T23:55:00Z".into());

        let updates = build_observation_updates(
            &current_record,
            Some(&previous_record),
            "2026-03-31T00:05:00Z",
        );

        assert_eq!(updates.domain_updates.len(), 2);
        assert_eq!(updates.domain_updates[0].day, "2026-03-30");
        assert_eq!(updates.domain_updates[0].upload, 7);
        assert_eq!(updates.domain_updates[0].download, 7);
        assert_eq!(updates.domain_updates[1].day, "2026-03-31");
        assert_eq!(updates.domain_updates[1].upload, 8);
        assert_eq!(updates.domain_updates[1].download, 8);
        assert_eq!(
            updates
                .domain_updates
                .iter()
                .map(|update| update.upload)
                .sum::<i64>(),
            15
        );
        assert_eq!(
            updates
                .domain_updates
                .iter()
                .map(|update| update.download)
                .sum::<i64>(),
            15
        );
        assert_eq!(updates.rule_updates.len(), 2);
        assert_eq!(updates.rule_updates[0].day, "2026-03-30");
        assert_eq!(updates.rule_updates[1].day, "2026-03-31");
        assert_eq!(updates.ip_traffic_updates.len(), 2);
        assert_eq!(updates.ip_traffic_updates[0].day, "2026-03-30");
        assert_eq!(updates.ip_traffic_updates[1].day, "2026-03-31");
    }

    #[test]
    fn build_observation_updates_split_first_seen_long_lived_connections() {
        let mut current_record = sample_record("same", "example.com", 24, 12);
        current_record.start_time = "2026-03-31T08:30:00Z".into();
        current_record.last_observed_at = Some("2026-03-31T10:30:00Z".into());

        let updates = build_observation_updates(&current_record, None, "2026-03-31T10:30:00Z");

        assert_eq!(updates.traffic_samples.len(), 3);
        assert_eq!(updates.traffic_samples[0].ts, "2026-03-31T08:00:00Z");
        assert_eq!(updates.traffic_samples[0].upload, 6);
        assert_eq!(updates.traffic_samples[0].download, 3);
        assert_eq!(updates.traffic_samples[0].conn_count, 1);
        assert_eq!(updates.traffic_samples[1].ts, "2026-03-31T09:00:00Z");
        assert_eq!(updates.traffic_samples[1].upload, 12);
        assert_eq!(updates.traffic_samples[1].download, 6);
        assert_eq!(updates.traffic_samples[1].conn_count, 0);
        assert_eq!(updates.traffic_samples[2].ts, "2026-03-31T10:00:00Z");
        assert_eq!(updates.traffic_samples[2].upload, 6);
        assert_eq!(updates.traffic_samples[2].download, 3);
        assert_eq!(updates.traffic_samples[2].conn_count, 0);

        assert_eq!(updates.domain_updates.len(), 1);
        assert_eq!(updates.domain_updates[0].day, "2026-03-31");
        assert_eq!(updates.domain_updates[0].hit_count, 1);
        assert_eq!(updates.domain_updates[0].upload, 24);
        assert_eq!(updates.domain_updates[0].download, 12);
        assert_eq!(updates.rule_updates.len(), 1);
        assert_eq!(updates.rule_updates[0].rule, "DOMAIN-SUFFIX");
        assert_eq!(updates.rule_updates[0].hit_count, 1);
        assert_eq!(updates.ip_traffic_updates.len(), 1);
        assert_eq!(updates.ip_traffic_updates[0].dst_ip, "1.1.1.1");
        assert_eq!(updates.ip_traffic_updates[0].day, "2026-03-31");
        assert_eq!(updates.ip_traffic_updates[0].upload, 24);
        assert_eq!(updates.ip_traffic_updates[0].download, 12);
    }

    #[test]
    fn build_observation_updates_normalizes_empty_rule_names() {
        let mut current_record = sample_record("same", "example.com", 12, 6);
        current_record.rule.clear();
        current_record.start_time = "2026-03-31T10:00:00Z".into();

        let updates = build_observation_updates(&current_record, None, "2026-03-31T10:30:00Z");

        assert_eq!(updates.rule_updates.len(), 1);
        assert_eq!(updates.rule_updates[0].rule, "UNKNOWN");
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
