use std::{
    collections::HashMap,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
    path::{Path, PathBuf},
    sync::Mutex,
    time::Duration,
};

use maxminddb::{geoip2, Reader};
use serde::{Deserialize, Serialize};
use tauri_plugin_sql::DbPool;
use tokio::sync::{Mutex as AsyncMutex, RwLock};
use tokio::time::Instant;
use tracing::warn;

use crate::db::{repo_geoip, DbError};

const DEFAULT_MIHOMO_CONFIG_DIR: &str = "~/.config/mihomo";
const COUNTRY_MMDB_FILE_NAME: &str = "Country.mmdb";
const IP_API_MIN_INTERVAL: Duration = Duration::from_millis(1_400);
const IP_API_FAILURE_RETRY_INTERVAL: Duration = Duration::from_secs(15 * 60);
const MISSING_MMDB_RETRY_INTERVAL: Duration = Duration::from_secs(30);

struct MmdbReaderState {
    path: PathBuf,
    reader: Reader<Vec<u8>>,
}

enum MmdbState {
    Unconfigured,
    Loaded(MmdbReaderState),
    Missing { path: PathBuf, checked_at: Instant },
}

#[derive(Debug, Default)]
struct IpApiRateLimiter {
    next_allowed_at: Option<Instant>,
}

pub struct GeoIpLookup {
    client: reqwest::Client,
    mmdb: RwLock<MmdbState>,
    ip_api_limiter: AsyncMutex<IpApiRateLimiter>,
    ip_api_failures: AsyncMutex<HashMap<String, Instant>>,
}

#[derive(Debug)]
pub struct GeoIpConfigState {
    config_dir: Mutex<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GeoLocation {
    pub ip: String,
    pub country: Option<String>,
    pub country_code: Option<String>,
    pub city: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct IpApiResponse {
    status: String,
    country: Option<String>,
    country_code: Option<String>,
    city: Option<String>,
    lat: Option<f64>,
    lon: Option<f64>,
}

impl GeoIpLookup {
    pub fn new(mmdb_path: Option<PathBuf>) -> Self {
        let client = match reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()
        {
            Ok(client) => client,
            Err(error) => {
                warn!("GeoIP HTTP 客户端初始化失败，回退到默认配置: {error}");
                reqwest::Client::new()
            }
        };

        let mmdb = match mmdb_path {
            Some(path) => match load_mmdb_reader(&path) {
                Some(reader) => MmdbState::Loaded(MmdbReaderState { path, reader }),
                None => MmdbState::Missing {
                    path,
                    checked_at: Instant::now(),
                },
            },
            None => MmdbState::Unconfigured,
        };

        Self {
            client,
            mmdb: RwLock::new(mmdb),
            ip_api_limiter: AsyncMutex::new(IpApiRateLimiter::default()),
            ip_api_failures: AsyncMutex::new(HashMap::new()),
        }
    }

    pub async fn lookup_many(
        &self,
        db: &DbPool,
        mmdb_path: Option<&Path>,
        ips: &[String],
        online_lookup_limit: usize,
    ) -> Result<HashMap<String, GeoLocation>, DbError> {
        if ips.is_empty() {
            return Ok(HashMap::new());
        }

        let cached_geos = repo_geoip::get_cached_geos(db, ips).await?;
        self.ensure_mmdb_reader(mmdb_path).await;

        let mut resolved_geos = HashMap::with_capacity(cached_geos.len());
        let mut mmdb_updates = Vec::new();
        let mut unresolved_public_ips = Vec::<(String, IpAddr)>::new();

        for ip in ips {
            let trimmed_ip = ip.trim();
            if trimmed_ip.is_empty() {
                continue;
            }

            if let Some(cached) = cached_geos.get(trimmed_ip) {
                if cached.is_resolved() {
                    resolved_geos.insert(trimmed_ip.to_string(), cached.clone());
                    continue;
                }
            }

            let ip_addr = match trimmed_ip.parse::<IpAddr>() {
                Ok(ip_addr) => ip_addr,
                Err(error) => {
                    warn!("GeoIP 跳过无效 IP {trimmed_ip}: {error}");
                    continue;
                }
            };

            if let Some(location) = self.lookup_mmdb_loaded(ip_addr, trimmed_ip).await {
                mmdb_updates.push(location.clone());
                resolved_geos.insert(trimmed_ip.to_string(), location);
                continue;
            }

            if is_public_ip(&ip_addr) {
                unresolved_public_ips.push((trimmed_ip.to_string(), ip_addr));
            }
        }

        if !mmdb_updates.is_empty() {
            repo_geoip::batch_cache_geo(db, &mmdb_updates).await?;
        }

        let online_candidates = self
            .select_online_lookup_candidates(unresolved_public_ips, online_lookup_limit)
            .await;
        let mut online_updates = Vec::new();
        for (ip, ip_addr) in online_candidates {
            if let Some(location) = self.lookup_ip_api(ip_addr, &ip).await {
                self.clear_ip_api_failure(&ip).await;
                resolved_geos.insert(ip, location.clone());
                online_updates.push(location);
            } else {
                self.record_ip_api_failure(&ip).await;
            }
        }

        if !online_updates.is_empty() {
            repo_geoip::batch_cache_geo(db, &online_updates).await?;
        }

        Ok(resolved_geos)
    }

    async fn select_online_lookup_candidates(
        &self,
        unresolved_public_ips: Vec<(String, IpAddr)>,
        online_lookup_limit: usize,
    ) -> Vec<(String, IpAddr)> {
        if online_lookup_limit == 0 || unresolved_public_ips.is_empty() {
            return Vec::new();
        }

        let now = Instant::now();
        let mut ip_api_failures = self.ip_api_failures.lock().await;
        retain_recent_ip_api_failures(&mut ip_api_failures, now);

        select_online_lookup_candidates_from_failures(
            unresolved_public_ips,
            &ip_api_failures,
            online_lookup_limit,
        )
    }

    async fn record_ip_api_failure(&self, ip: &str) {
        let mut ip_api_failures = self.ip_api_failures.lock().await;
        ip_api_failures.insert(ip.to_string(), Instant::now());
    }

    async fn clear_ip_api_failure(&self, ip: &str) {
        let mut ip_api_failures = self.ip_api_failures.lock().await;
        ip_api_failures.remove(ip);
    }

    async fn lookup_mmdb_loaded(&self, ip_addr: IpAddr, ip: &str) -> Option<GeoLocation> {
        let guard = self.mmdb.read().await;
        let MmdbState::Loaded(state) = &*guard else {
            return None;
        };

        Self::lookup_mmdb_with_reader(&state.reader, ip_addr, ip)
    }

    async fn lookup_ip_api(&self, ip_addr: IpAddr, ip: &str) -> Option<GeoLocation> {
        if !is_public_ip(&ip_addr) {
            return None;
        }

        self.wait_for_ip_api_slot().await;

        let url = format!(
            "http://ip-api.com/json/{}?fields=status,country,countryCode,city,lat,lon",
            urlencoding::encode(ip)
        );
        let response = match self.client.get(url).send().await {
            Ok(response) => response,
            Err(error) => {
                warn!("IP-API GeoIP 回退失败: ip={ip}, error={error}");
                return None;
            }
        };

        if !response.status().is_success() {
            warn!(
                "IP-API GeoIP 回退返回异常状态: ip={ip}, status={}",
                response.status()
            );
            return None;
        }

        let payload = match response.json::<IpApiResponse>().await {
            Ok(payload) => payload,
            Err(error) => {
                warn!("IP-API GeoIP 响应解析失败: ip={ip}, error={error}");
                return None;
            }
        };

        if payload.status != "success" {
            return None;
        }

        if payload.country.is_none() && payload.country_code.is_none() {
            return None;
        }

        Some(GeoLocation {
            ip: ip.to_string(),
            country: payload.country,
            country_code: payload.country_code,
            city: payload.city,
            latitude: payload.lat,
            longitude: payload.lon,
        })
    }

    async fn ensure_mmdb_reader(&self, mmdb_path: Option<&Path>) {
        let Some(path) = mmdb_path else {
            let mut guard = self.mmdb.write().await;
            *guard = MmdbState::Unconfigured;
            return;
        };

        {
            let guard = self.mmdb.read().await;
            match &*guard {
                MmdbState::Loaded(state) if state.path == path => return,
                MmdbState::Missing {
                    path: missing_path,
                    checked_at,
                } if missing_path == path && checked_at.elapsed() < MISSING_MMDB_RETRY_INTERVAL => {
                    return;
                }
                _ => {}
            }
        }

        let next_state = match tokio::fs::read(path).await {
            Ok(buffer) => match Reader::from_source(buffer) {
                Ok(reader) => MmdbState::Loaded(MmdbReaderState {
                    path: path.to_path_buf(),
                    reader,
                }),
                Err(error) => {
                    warn!(
                        "Country.mmdb 加载失败: path={}, error={error}",
                        path.display()
                    );
                    MmdbState::Missing {
                        path: path.to_path_buf(),
                        checked_at: Instant::now(),
                    }
                }
            },
            Err(error) => {
                warn!(
                    "Country.mmdb 不可用: path={}, error={error}",
                    path.display()
                );
                MmdbState::Missing {
                    path: path.to_path_buf(),
                    checked_at: Instant::now(),
                }
            }
        };

        let mut guard = self.mmdb.write().await;
        *guard = next_state;
    }

    async fn wait_for_ip_api_slot(&self) {
        loop {
            let wait_duration = {
                let mut guard = self.ip_api_limiter.lock().await;
                let now = Instant::now();

                match guard.next_allowed_at {
                    Some(next_allowed_at) if next_allowed_at > now => Some(next_allowed_at - now),
                    _ => {
                        guard.next_allowed_at = Some(now + IP_API_MIN_INTERVAL);
                        None
                    }
                }
            };

            match wait_duration {
                Some(duration) => tokio::time::sleep(duration).await,
                None => return,
            }
        }
    }

    fn lookup_mmdb_with_reader(
        reader: &Reader<Vec<u8>>,
        ip_addr: IpAddr,
        ip: &str,
    ) -> Option<GeoLocation> {
        let country = match reader.lookup::<geoip2::Country<'_>>(ip_addr) {
            Ok(country) => country,
            Err(error) => {
                warn!("MMDB GeoIP 查询失败: ip={ip}, error={error}");
                return None;
            }
        };

        let country_name = country
            .country
            .as_ref()
            .and_then(|country_info| country_info.names.as_ref())
            .and_then(|names| names.get("en"))
            .map(ToString::to_string);
        let country_code = country
            .country
            .and_then(|country_info| country_info.iso_code.map(ToString::to_string));

        if country_name.is_none() && country_code.is_none() {
            return None;
        }

        Some(GeoLocation {
            ip: ip.to_string(),
            country: country_name,
            country_code,
            city: None,
            latitude: None,
            longitude: None,
        })
    }
}

impl GeoIpConfigState {
    pub fn new(config_dir: String) -> Self {
        Self {
            config_dir: Mutex::new(config_dir),
        }
    }

    pub fn config_dir(&self) -> String {
        match self.config_dir.lock() {
            Ok(guard) => guard.clone(),
            Err(_) => default_mihomo_config_dir(),
        }
    }

    pub fn set_config_dir(&self, config_dir: impl Into<String>) {
        match self.config_dir.lock() {
            Ok(mut guard) => {
                *guard = config_dir.into();
            }
            Err(error) => {
                warn!("GeoIP 配置目录状态已中毒，忽略更新: {error}");
            }
        }
    }
}

impl GeoLocation {
    #[must_use]
    pub fn is_resolved(&self) -> bool {
        self.country.is_some() || self.country_code.is_some()
    }
}

pub fn default_mihomo_config_dir() -> String {
    DEFAULT_MIHOMO_CONFIG_DIR.to_string()
}

pub fn resolve_country_mmdb_path(config_dir: &str) -> Option<PathBuf> {
    let config_dir = expand_tilde(config_dir);
    let candidate = Path::new(&config_dir).join(COUNTRY_MMDB_FILE_NAME);

    candidate.exists().then_some(candidate)
}

fn load_mmdb_reader(path: &Path) -> Option<Reader<Vec<u8>>> {
    let buffer = match std::fs::read(path) {
        Ok(buffer) => buffer,
        Err(error) => {
            warn!(
                "Country.mmdb 初始化失败: path={}, error={error}",
                path.display()
            );
            return None;
        }
    };

    match Reader::from_source(buffer) {
        Ok(reader) => Some(reader),
        Err(error) => {
            warn!(
                "Country.mmdb 初始化解析失败: path={}, error={error}",
                path.display()
            );
            None
        }
    }
}

fn expand_tilde(path: &str) -> String {
    if let Some(rest) = path.strip_prefix('~') {
        if let Some(home) = dirs::home_dir() {
            return format!("{}{rest}", home.display());
        }
    }

    path.to_string()
}

fn is_public_ip(ip_addr: &IpAddr) -> bool {
    match ip_addr {
        IpAddr::V4(ipv4) => is_public_ipv4(ipv4),
        IpAddr::V6(ipv6) => is_public_ipv6(ipv6),
    }
}

fn is_public_ipv4(ipv4: &Ipv4Addr) -> bool {
    !ipv4.is_private()
        && !ipv4.is_loopback()
        && !ipv4.is_link_local()
        && !ipv4.is_broadcast()
        && !ipv4.is_documentation()
        && !ipv4.is_unspecified()
        && !ipv4.is_multicast()
}

fn is_public_ipv6(ipv6: &Ipv6Addr) -> bool {
    !ipv6.is_loopback()
        && !ipv6.is_unique_local()
        && !ipv6.is_unicast_link_local()
        && !ipv6.is_unspecified()
        && !ipv6.is_multicast()
}

fn retain_recent_ip_api_failures(ip_api_failures: &mut HashMap<String, Instant>, now: Instant) {
    ip_api_failures.retain(|_, failed_at| is_recent_ip_api_failure(*failed_at, now));
}

fn is_recent_ip_api_failure(failed_at: Instant, now: Instant) -> bool {
    match now.checked_duration_since(failed_at) {
        Some(duration) => duration < IP_API_FAILURE_RETRY_INTERVAL,
        None => true,
    }
}

fn select_online_lookup_candidates_from_failures(
    unresolved_public_ips: Vec<(String, IpAddr)>,
    ip_api_failures: &HashMap<String, Instant>,
    online_lookup_limit: usize,
) -> Vec<(String, IpAddr)> {
    let mut candidates = Vec::with_capacity(online_lookup_limit.min(unresolved_public_ips.len()));

    for (ip, ip_addr) in unresolved_public_ips {
        if ip_api_failures.contains_key(&ip) {
            continue;
        }

        candidates.push((ip, ip_addr));
        if candidates.len() >= online_lookup_limit {
            break;
        }
    }

    candidates
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_country_mmdb_path_returns_none_for_missing_file() {
        let unique_dir = format!(
            "{}\\ClashMind-GeoIp-{}",
            std::env::temp_dir().display(),
            std::process::id()
        );

        assert!(resolve_country_mmdb_path(&unique_dir).is_none());
    }

    #[test]
    fn geo_location_reports_resolution_state() {
        let unresolved = GeoLocation {
            ip: "127.0.0.1".into(),
            country: None,
            country_code: None,
            city: None,
            latitude: None,
            longitude: None,
        };
        let resolved = GeoLocation {
            ip: "1.1.1.1".into(),
            country: Some("Australia".into()),
            country_code: Some("AU".into()),
            city: None,
            latitude: None,
            longitude: None,
        };

        assert!(!unresolved.is_resolved());
        assert!(resolved.is_resolved());
    }

    #[test]
    fn select_online_lookup_candidates_skips_recent_failures_and_preserves_order() {
        let now = Instant::now();
        let mut ip_api_failures = HashMap::new();
        ip_api_failures.insert("1.1.1.1".to_string(), now);
        ip_api_failures.insert("8.8.8.8".to_string(), now);

        let candidates = select_online_lookup_candidates_from_failures(
            vec![
                ("1.1.1.1".to_string(), IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1))),
                ("8.8.8.8".to_string(), IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))),
                ("9.9.9.9".to_string(), IpAddr::V4(Ipv4Addr::new(9, 9, 9, 9))),
                ("1.0.0.1".to_string(), IpAddr::V4(Ipv4Addr::new(1, 0, 0, 1))),
            ],
            &ip_api_failures,
            2,
        );

        assert_eq!(candidates.len(), 2);
        assert_eq!(candidates[0].0, "9.9.9.9");
        assert_eq!(candidates[1].0, "1.0.0.1");
    }

    #[test]
    fn retain_recent_ip_api_failures_drops_expired_entries() {
        let now = Instant::now();
        let expired_failure = now - IP_API_FAILURE_RETRY_INTERVAL - Duration::from_secs(1);
        let mut ip_api_failures = HashMap::new();
        ip_api_failures.insert("1.1.1.1".to_string(), expired_failure);
        ip_api_failures.insert("8.8.8.8".to_string(), now);

        retain_recent_ip_api_failures(&mut ip_api_failures, now);

        assert!(!ip_api_failures.contains_key("1.1.1.1"));
        assert!(ip_api_failures.contains_key("8.8.8.8"));
    }
}
