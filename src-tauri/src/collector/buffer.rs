//! Buffered connection writes with capacity and time-based flush triggers.

use std::{
    collections::HashSet,
    mem,
    time::{Duration, Instant},
};

use super::ws_client::ConnectionRecord;

pub const DEFAULT_BATCH_CAPACITY: usize = 5_000;
pub const DEFAULT_FLUSH_INTERVAL: Duration = Duration::from_secs(30);

/// Buffers connection records until they should be persisted in batch.
#[derive(Debug)]
pub struct BatchBuffer {
    buffer: Vec<ConnectionRecord>,
    buffered_ids: HashSet<String>,
    capacity: usize,
    flush_interval: Duration,
    last_flush: Instant,
}

impl BatchBuffer {
    #[must_use]
    pub fn new(capacity: usize, flush_interval: Duration) -> Self {
        let normalized_capacity = capacity.max(1);

        Self {
            buffer: Vec::with_capacity(normalized_capacity),
            buffered_ids: HashSet::with_capacity(normalized_capacity),
            capacity: normalized_capacity,
            flush_interval,
            last_flush: Instant::now(),
        }
    }

    /// Pushes a record into the buffer and returns a drained batch when capacity is reached.
    pub fn push(&mut self, record: ConnectionRecord) -> Option<Vec<ConnectionRecord>> {
        self.buffered_ids.insert(record.id.clone());
        self.buffer.push(record);

        if self.buffer.len() >= self.capacity {
            return Some(self.flush());
        }

        None
    }

    /// Returns whether the timer-based flush threshold has been reached.
    #[must_use]
    pub fn should_flush(&self) -> bool {
        !self.buffer.is_empty() && self.flush_due()
    }

    #[must_use]
    pub fn flush_due(&self) -> bool {
        self.last_flush.elapsed() >= self.flush_interval
    }

    /// Drains all buffered records and resets the flush timer.
    #[must_use]
    pub fn flush(&mut self) -> Vec<ConnectionRecord> {
        self.last_flush = Instant::now();
        self.buffered_ids.clear();

        let mut drained = Vec::with_capacity(self.capacity.max(self.buffer.len()));
        mem::swap(&mut drained, &mut self.buffer);
        drained
    }

    /// Restores a drained batch after a failed persistence attempt.
    pub fn restore(&mut self, mut records: Vec<ConnectionRecord>) {
        if self.buffer.is_empty() {
            self.buffered_ids.clear();
            for r in &records {
                self.buffered_ids.insert(r.id.clone());
            }
            self.buffer = records;
            return;
        }

        records.append(&mut self.buffer);
        self.buffered_ids.clear();
        for r in &records {
            self.buffered_ids.insert(r.id.clone());
        }
        self.buffer = records;
    }

    #[must_use]
    pub fn contains_connection(&self, id: &str) -> bool {
        self.buffered_ids.contains(id)
    }

    #[must_use]
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn len(&self) -> usize {
        self.buffer.len()
    }
}

#[cfg(test)]
mod tests {
    use std::{thread, time::Duration};

    use super::*;

    fn sample_record(id: &str) -> ConnectionRecord {
        ConnectionRecord {
            id: id.to_string(),
            host: "example.com".into(),
            dst_ip: Some("1.1.1.1".into()),
            dst_port: Some(443),
            src_ip: Some("127.0.0.1".into()),
            src_port: Some(9000),
            network: "tcp".into(),
            conn_type: "HTTPS".into(),
            rule: "DOMAIN-SUFFIX".into(),
            rule_payload: Some("example.com".into()),
            proxy_chain: "[\"DIRECT\"]".into(),
            upload: 1,
            download: 2,
            start_time: "2026-03-30T10:00:00.000Z".into(),
            last_observed_at: Some("2026-03-30T10:00:00.000Z".into()),
        }
    }

    #[test]
    fn push_flushes_when_capacity_is_reached() {
        let mut buffer = BatchBuffer::new(2, Duration::from_secs(30));

        assert!(buffer.push(sample_record("first")).is_none());
        let drained = buffer.push(sample_record("second"));

        assert!(drained.is_some());
        let Some(drained) = drained else {
            panic!("capacity flush should return buffered records");
        };

        assert_eq!(drained.len(), 2);
        assert_eq!(buffer.len(), 0);
    }

    #[test]
    fn should_flush_only_when_interval_elapsed_and_buffer_has_records() {
        let mut buffer = BatchBuffer::new(4, Duration::from_millis(5));

        assert!(!buffer.should_flush());
        assert!(buffer.push(sample_record("first")).is_none());
        assert!(!buffer.should_flush());

        thread::sleep(Duration::from_millis(8));

        assert!(buffer.should_flush());
    }

    #[test]
    fn restore_puts_records_back_into_the_buffer() {
        let mut buffer = BatchBuffer::new(4, Duration::from_secs(30));

        assert!(buffer.push(sample_record("first")).is_none());
        let drained = buffer.flush();
        buffer.restore(drained);

        assert_eq!(buffer.len(), 1);
        assert!(buffer.contains_connection("first"));
    }
}
