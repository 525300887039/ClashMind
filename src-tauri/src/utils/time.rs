//! Shared UTC time helpers for collector and database modules.

use chrono::{
    DateTime, Datelike, Duration as ChronoDuration, DurationRound, SecondsFormat, TimeZone, Utc,
};

/// Formats a UTC timestamp as an ISO 8601 string with second precision.
#[must_use]
pub fn format_utc(value: DateTime<Utc>) -> String {
    value.to_rfc3339_opts(SecondsFormat::Secs, true)
}

/// Parses an RFC 3339 timestamp string into UTC.
#[must_use]
pub fn parse_rfc3339(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|timestamp| timestamp.with_timezone(&Utc))
}

/// Truncates a timestamp to the start of its UTC hour.
#[must_use]
pub fn floor_to_hour(value: DateTime<Utc>) -> DateTime<Utc> {
    match value.duration_trunc(ChronoDuration::hours(1)) {
        Ok(timestamp) => timestamp,
        Err(_) => value,
    }
}

/// Truncates a timestamp to the start of its UTC day.
#[must_use]
pub fn floor_to_day(value: DateTime<Utc>) -> DateTime<Utc> {
    match Utc.with_ymd_and_hms(value.year(), value.month(), value.day(), 0, 0, 0) {
        chrono::LocalResult::Single(timestamp) => timestamp,
        _ => value,
    }
}

/// Returns the next UTC hour boundary after the provided timestamp.
#[must_use]
pub fn next_hour_boundary(value: &DateTime<Utc>) -> DateTime<Utc> {
    floor_to_hour(*value) + ChronoDuration::hours(1)
}

/// Returns the next UTC day boundary after the provided timestamp.
#[must_use]
pub fn next_day_boundary(value: &DateTime<Utc>) -> DateTime<Utc> {
    floor_to_day(*value) + ChronoDuration::days(1)
}

/// Returns the ISO 8601 hour bucket key for the provided timestamp.
#[must_use]
pub fn hour_bucket_key(value: &DateTime<Utc>) -> String {
    format_utc(floor_to_hour(*value))
}

/// Returns the day bucket key for the provided timestamp.
#[must_use]
pub fn day_bucket_key(value: &DateTime<Utc>) -> String {
    value.date_naive().to_string()
}

/// Shifts a timestamp forward by one hour.
#[must_use]
pub fn shift_hour(value: DateTime<Utc>) -> DateTime<Utc> {
    value + ChronoDuration::hours(1)
}

/// Shifts a timestamp forward by one day.
#[must_use]
pub fn shift_day(value: DateTime<Utc>) -> DateTime<Utc> {
    value + ChronoDuration::days(1)
}
