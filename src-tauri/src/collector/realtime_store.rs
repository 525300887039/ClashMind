//! In-memory realtime cache for active and recently closed connections.

use std::{collections::HashMap, sync::Arc};

use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::debug;

use super::ws_client::ConnectionRecord;

const MAX_RECENT_CLOSED: usize = 1_000;
const TOP_ITEMS_LIMIT: usize = 10;

/// Cached realtime summary derived from active connections.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct RealtimeSummary {
    pub active_count: usize,
    pub total_upload: i64,
    pub total_download: i64,
    pub top_domains: Vec<(String, i64)>,
    pub top_rules: Vec<(String, usize)>,
}

/// Shared in-memory store for active and recently closed connections.
#[derive(Debug, Clone)]
pub struct RealtimeStore {
    active: Arc<RwLock<HashMap<String, ConnectionRecord>>>,
    recent_closed: Arc<RwLock<Vec<ConnectionRecord>>>,
    summary: Arc<RwLock<RealtimeSummary>>,
}

impl RealtimeStore {
    /// Creates an empty realtime store.
    #[must_use]
    pub fn new() -> Self {
        Self {
            active: Arc::new(RwLock::new(HashMap::new())),
            recent_closed: Arc::new(RwLock::new(Vec::new())),
            summary: Arc::new(RwLock::new(RealtimeSummary::default())),
        }
    }

    /// Replaces the active snapshot and tracks opened, updated, and closed connections.
    pub async fn update_snapshot(&self, connections: Vec<ConnectionRecord>) {
        let mut next_active = HashMap::with_capacity(connections.len());
        for connection in connections {
            next_active.insert(connection.id.clone(), connection);
        }

        let next_summary = build_summary(next_active.values());

        let (opened_count, updated_count, closed_records) = {
            let mut active = self.active.write().await;
            let mut opened_count = 0usize;
            let mut updated_count = 0usize;

            for (connection_id, current_record) in &next_active {
                match active.get(connection_id) {
                    Some(previous_record) if previous_record != current_record => {
                        updated_count += 1;
                    }
                    None => {
                        opened_count += 1;
                    }
                    Some(_) => {}
                }
            }

            let mut closed_records = Vec::new();
            for (connection_id, previous_record) in active.iter() {
                if !next_active.contains_key(connection_id) {
                    closed_records.push(previous_record.clone());
                }
            }

            *active = next_active;
            (opened_count, updated_count, closed_records)
        };

        let closed_count = closed_records.len();
        self.push_recent_closed(closed_records).await;

        {
            let mut summary = self.summary.write().await;
            *summary = next_summary;
        }

        if opened_count > 0 || updated_count > 0 || closed_count > 0 {
            debug!(
                opened = opened_count,
                updated = updated_count,
                closed = closed_count,
                "realtime store snapshot updated"
            );
        }
    }

    /// Returns the current active connections ordered by id for stable reads.
    pub async fn get_active_connections(&self) -> Vec<ConnectionRecord> {
        let active = self.active.read().await;
        let mut records: Vec<_> = active.values().cloned().collect();
        records.sort_by(|left, right| left.id.cmp(&right.id));
        records
    }

    /// Returns the cached realtime summary.
    pub async fn get_summary(&self) -> RealtimeSummary {
        let summary = self.summary.read().await;
        summary.clone()
    }

    /// Marks the provided active connection ids as closed.
    #[cfg_attr(not(test), allow(dead_code))]
    pub async fn mark_closed(&self, ids: Vec<String>) {
        if ids.is_empty() {
            return;
        }

        let (closed_records, next_summary) = {
            let mut active = self.active.write().await;
            let mut closed_records = Vec::new();

            for connection_id in ids {
                if let Some(record) = active.remove(&connection_id) {
                    closed_records.push(record);
                }
            }

            let next_summary = build_summary(active.values());
            (closed_records, next_summary)
        };

        self.push_recent_closed(closed_records).await;

        {
            let mut summary = self.summary.write().await;
            *summary = next_summary;
        }
    }

    /// Clears realtime active state without treating current connections as closed.
    pub async fn clear_active(&self) {
        {
            let mut active = self.active.write().await;
            active.clear();
        }

        {
            let mut summary = self.summary.write().await;
            *summary = RealtimeSummary::default();
        }
    }

    /// Clears all realtime state, including recent closed history.
    pub async fn reset(&self) {
        self.clear_active().await;

        let mut recent_closed = self.recent_closed.write().await;
        recent_closed.clear();
    }

    async fn push_recent_closed(&self, mut records: Vec<ConnectionRecord>) {
        if records.is_empty() {
            return;
        }

        let mut recent_closed = self.recent_closed.write().await;
        recent_closed.append(&mut records);

        if recent_closed.len() > MAX_RECENT_CLOSED {
            let overflow = recent_closed.len() - MAX_RECENT_CLOSED;
            recent_closed.drain(..overflow);
        }
    }
}

impl Default for RealtimeStore {
    fn default() -> Self {
        Self::new()
    }
}

fn build_summary<'a, I>(connections: I) -> RealtimeSummary
where
    I: IntoIterator<Item = &'a ConnectionRecord>,
{
    let mut active_count = 0usize;
    let mut total_upload = 0i64;
    let mut total_download = 0i64;
    let mut domain_traffic: HashMap<String, i64> = HashMap::new();
    let mut rule_counts: HashMap<String, usize> = HashMap::new();

    for connection in connections {
        active_count += 1;

        let upload = connection.upload.max(0);
        let download = connection.download.max(0);
        total_upload = total_upload.saturating_add(upload);
        total_download = total_download.saturating_add(download);

        let traffic = upload.saturating_add(download);
        let host = connection.host.trim();
        if !host.is_empty() {
            let entry = domain_traffic.entry(host.to_string()).or_insert(0);
            *entry = entry.saturating_add(traffic);
        }

        let rule = connection.rule.trim();
        if !rule.is_empty() {
            let entry = rule_counts.entry(rule.to_string()).or_insert(0);
            *entry += 1;
        }
    }

    RealtimeSummary {
        active_count,
        total_upload,
        total_download,
        top_domains: sort_top_domains(domain_traffic),
        top_rules: sort_top_rules(rule_counts),
    }
}

fn sort_top_domains(domain_traffic: HashMap<String, i64>) -> Vec<(String, i64)> {
    let mut items: Vec<_> = domain_traffic.into_iter().collect();
    items.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    items.truncate(TOP_ITEMS_LIMIT);
    items
}

fn sort_top_rules(rule_counts: HashMap<String, usize>) -> Vec<(String, usize)> {
    let mut items: Vec<_> = rule_counts.into_iter().collect();
    items.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    items.truncate(TOP_ITEMS_LIMIT);
    items
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_record(
        id: &str,
        host: &str,
        rule: &str,
        upload: i64,
        download: i64,
    ) -> ConnectionRecord {
        ConnectionRecord {
            id: id.to_string(),
            host: host.to_string(),
            dst_ip: Some("1.1.1.1".into()),
            dst_port: Some(443),
            src_ip: Some("127.0.0.1".into()),
            src_port: Some(9000),
            network: "tcp".into(),
            conn_type: "HTTPS".into(),
            rule: rule.to_string(),
            rule_payload: Some(host.to_string()),
            proxy_chain: "[\"DIRECT\"]".into(),
            upload,
            download,
            start_time: "2026-03-30T10:00:00.000Z".into(),
            last_observed_at: Some("2026-03-30T10:00:00.000Z".into()),
        }
    }

    #[tokio::test]
    async fn update_snapshot_tracks_changes_and_rebuilds_summary() {
        let store = RealtimeStore::new();
        store
            .update_snapshot(vec![
                sample_record("alpha", "a.example", "MATCH", 10, 20),
                sample_record("beta", "b.example", "RULE-SET", 30, 40),
            ])
            .await;

        store
            .update_snapshot(vec![
                sample_record("alpha", "a.example", "MATCH", 15, 25),
                sample_record("gamma", "a.example", "MATCH", 5, 10),
            ])
            .await;

        let active = store.get_active_connections().await;
        let summary = store.get_summary().await;
        let recent_closed = store.recent_closed.read().await.clone();

        assert_eq!(active.len(), 2);
        assert_eq!(active[0].id, "alpha");
        assert_eq!(active[1].id, "gamma");
        assert_eq!(summary.active_count, 2);
        assert_eq!(summary.total_upload, 20);
        assert_eq!(summary.total_download, 35);
        assert_eq!(summary.top_domains, vec![("a.example".to_string(), 55)]);
        assert_eq!(summary.top_rules, vec![("MATCH".to_string(), 2)]);
        assert_eq!(recent_closed.len(), 1);
        assert_eq!(recent_closed[0].id, "beta");
    }

    #[tokio::test]
    async fn mark_closed_removes_active_connections_and_updates_summary() {
        let store = RealtimeStore::new();
        store
            .update_snapshot(vec![
                sample_record("alpha", "a.example", "MATCH", 10, 20),
                sample_record("beta", "b.example", "RULE-SET", 30, 40),
            ])
            .await;

        store.mark_closed(vec!["alpha".to_string()]).await;

        let active = store.get_active_connections().await;
        let summary = store.get_summary().await;
        let recent_closed = store.recent_closed.read().await.clone();

        assert_eq!(active.len(), 1);
        assert_eq!(active[0].id, "beta");
        assert_eq!(summary.active_count, 1);
        assert_eq!(summary.total_upload, 30);
        assert_eq!(summary.total_download, 40);
        assert_eq!(recent_closed.len(), 1);
        assert_eq!(recent_closed[0].id, "alpha");
    }

    #[tokio::test]
    async fn recent_closed_uses_fifo_eviction() {
        let store = RealtimeStore::new();
        let mut initial_snapshot = Vec::with_capacity(MAX_RECENT_CLOSED + 2);
        let mut ids = Vec::with_capacity(MAX_RECENT_CLOSED + 2);

        for index in 0..(MAX_RECENT_CLOSED + 2) {
            let id = format!("conn-{index}");
            initial_snapshot.push(sample_record(
                &id,
                "example.com",
                "MATCH",
                index as i64,
                index as i64,
            ));
            ids.push(id);
        }

        store.update_snapshot(initial_snapshot).await;

        for id in ids {
            store.mark_closed(vec![id]).await;
        }

        let recent_closed = store.recent_closed.read().await.clone();

        assert_eq!(recent_closed.len(), MAX_RECENT_CLOSED);
        assert_eq!(recent_closed[0].id, "conn-2");
        assert_eq!(recent_closed[MAX_RECENT_CLOSED - 1].id, "conn-1001");
    }

    #[tokio::test]
    async fn reset_clears_active_state_and_summary() {
        let store = RealtimeStore::new();
        store
            .update_snapshot(vec![sample_record("alpha", "a.example", "MATCH", 10, 20)])
            .await;

        store.reset().await;

        let active = store.get_active_connections().await;
        let summary = store.get_summary().await;
        let recent_closed = store.recent_closed.read().await.clone();

        assert!(active.is_empty());
        assert_eq!(summary, RealtimeSummary::default());
        assert!(recent_closed.is_empty());
    }

    #[tokio::test]
    async fn clear_active_preserves_recent_closed_history() {
        let store = RealtimeStore::new();
        store
            .update_snapshot(vec![sample_record("alpha", "a.example", "MATCH", 10, 20)])
            .await;
        store.mark_closed(vec!["alpha".to_string()]).await;

        store
            .update_snapshot(vec![sample_record("beta", "b.example", "RULE-SET", 30, 40)])
            .await;
        store.clear_active().await;

        let active = store.get_active_connections().await;
        let summary = store.get_summary().await;
        let recent_closed = store.recent_closed.read().await.clone();

        assert!(active.is_empty());
        assert_eq!(summary, RealtimeSummary::default());
        assert_eq!(recent_closed.len(), 1);
        assert_eq!(recent_closed[0].id, "alpha");
    }
}
