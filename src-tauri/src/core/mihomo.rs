use std::collections::HashMap;

use reqwest::Client;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum MihomoError {
    #[error("HTTP 请求失败: {0}")]
    Request(#[from] reqwest::Error),
    #[error("API 返回错误: {0}")]
    Api(String),
}

impl Serialize for MihomoError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DelayHistory {
    pub time: String,
    pub delay: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProxyNode {
    pub name: String,
    #[serde(rename = "type")]
    pub proxy_type: String,
    pub alive: bool,
    #[serde(default)]
    pub delay: i64,
    #[serde(default)]
    pub history: Vec<DelayHistory>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ProxyGroup {
    pub name: String,
    #[serde(rename = "type")]
    pub group_type: String,
    #[serde(default)]
    pub now: String,
    #[serde(default)]
    pub all: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Rule {
    #[serde(rename = "type")]
    pub rule_type: String,
    pub payload: String,
    pub proxy: String,
}

pub struct MihomoClient {
    client: Client,
    base_url: String,
    secret: String,
}

impl MihomoClient {
    pub fn new(base_url: &str, secret: &str) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
            secret: secret.to_string(),
        }
    }

    pub fn update_connection(&mut self, base_url: &str, secret: &str) {
        self.base_url = base_url.trim_end_matches('/').to_string();
        self.secret = secret.to_string();
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    fn request(&self, method: reqwest::Method, path: &str) -> reqwest::RequestBuilder {
        let mut req = self.client.request(method, self.url(path));
        if !self.secret.is_empty() {
            req = req.header("Authorization", format!("Bearer {}", self.secret));
        }
        req
    }

    pub async fn get_proxies(&self) -> Result<serde_json::Value, MihomoError> {
        let resp = self.request(reqwest::Method::GET, "/proxies").send().await?;
        Ok(resp.json().await?)
    }

    pub async fn switch_proxy(&self, group: &str, name: &str) -> Result<(), MihomoError> {
        let resp = self
            .request(reqwest::Method::PUT, &format!("/proxies/{}", group))
            .json(&serde_json::json!({ "name": name }))
            .send()
            .await?;
        if resp.status().is_success() {
            Ok(())
        } else {
            Err(MihomoError::Api(resp.text().await.unwrap_or_default()))
        }
    }

    pub async fn test_delay(
        &self,
        name: &str,
        url: &str,
        timeout: u32,
    ) -> Result<u32, MihomoError> {
        let resp = self
            .request(
                reqwest::Method::GET,
                &format!("/proxies/{}/delay", name),
            )
            .query(&[("url", url), ("timeout", &timeout.to_string())])
            .send()
            .await?;
        let data: serde_json::Value = resp.json().await?;
        data.get("delay")
            .and_then(|v| v.as_u64())
            .map(|v| v as u32)
            .ok_or_else(|| MihomoError::Api("无效的延迟响应".into()))
    }

    pub async fn test_group_delay(
        &self,
        group: &str,
        url: &str,
        timeout: u32,
    ) -> Result<HashMap<String, u32>, MihomoError> {
        let resp = self
            .request(
                reqwest::Method::GET,
                &format!("/group/{}/delay", group),
            )
            .query(&[("url", url), ("timeout", &timeout.to_string())])
            .send()
            .await?;
        Ok(resp.json().await?)
    }

    pub async fn get_rules(&self) -> Result<serde_json::Value, MihomoError> {
        let resp = self.request(reqwest::Method::GET, "/rules").send().await?;
        Ok(resp.json().await?)
    }

    pub async fn get_configs(&self) -> Result<serde_json::Value, MihomoError> {
        let resp = self.request(reqwest::Method::GET, "/configs").send().await?;
        Ok(resp.json().await?)
    }

    pub async fn patch_configs(
        &self,
        payload: serde_json::Value,
    ) -> Result<(), MihomoError> {
        let resp = self
            .request(reqwest::Method::PATCH, "/configs")
            .json(&payload)
            .send()
            .await?;
        if resp.status().is_success() {
            Ok(())
        } else {
            Err(MihomoError::Api(resp.text().await.unwrap_or_default()))
        }
    }

    pub async fn close_connection(&self, id: &str) -> Result<(), MihomoError> {
        self.request(reqwest::Method::DELETE, &format!("/connections/{}", id))
            .send()
            .await?;
        Ok(())
    }

    pub async fn close_all_connections(&self) -> Result<(), MihomoError> {
        self.request(reqwest::Method::DELETE, "/connections")
            .send()
            .await?;
        Ok(())
    }

    pub async fn get_connections(&self) -> Result<serde_json::Value, MihomoError> {
        let resp = self
            .request(reqwest::Method::GET, "/connections")
            .send()
            .await?;
        Ok(resp.json().await?)
    }

    pub async fn get_version(&self) -> Result<serde_json::Value, MihomoError> {
        let resp = self.request(reqwest::Method::GET, "/version").send().await?;
        Ok(resp.json().await?)
    }

    pub async fn reload_configs(&self) -> Result<(), MihomoError> {
        let resp = self
            .request(reqwest::Method::PUT, "/configs?force=true")
            .json(&serde_json::json!({}))
            .send()
            .await?;
        if resp.status().is_success() {
            Ok(())
        } else {
            Err(MihomoError::Api(resp.text().await.unwrap_or_default()))
        }
    }
}
