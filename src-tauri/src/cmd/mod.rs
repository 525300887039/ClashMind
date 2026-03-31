use crate::core::mihomo::MihomoClient;

pub mod collector;
pub mod config;
pub mod proxy;
pub mod sidecar;
pub mod stats;
pub mod system;

pub struct MihomoState {
    pub client: tokio::sync::Mutex<MihomoClient>,
}
