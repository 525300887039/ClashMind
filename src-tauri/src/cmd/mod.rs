use crate::core::mihomo::MihomoClient;

pub mod config;
pub mod proxy;
pub mod sidecar;
pub mod system;

pub struct MihomoState {
    pub client: MihomoClient,
}
