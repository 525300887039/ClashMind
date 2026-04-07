use sysproxy::Sysproxy;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SysproxyError {
    #[error("系统代理设置失败: {0}")]
    SetFailed(String),
    #[error("获取系统代理失败: {0}")]
    GetFailed(String),
}

crate::utils::impl_serialize_display!(SysproxyError);

pub fn set_system_proxy(enable: bool, host: &str, port: u16) -> Result<(), SysproxyError> {
    if enable {
        let proxy = Sysproxy {
            enable: true,
            host: host.into(),
            port,
            bypass: "localhost,127.0.0.1".into(),
        };
        proxy
            .set_system_proxy()
            .map_err(|e| SysproxyError::SetFailed(e.to_string()))
    } else {
        let proxy = Sysproxy {
            enable: false,
            host: host.into(),
            port,
            bypass: "localhost,127.0.0.1".into(),
        };
        proxy
            .set_system_proxy()
            .map_err(|e| SysproxyError::SetFailed(e.to_string()))
    }
}

pub fn get_system_proxy() -> Result<Sysproxy, SysproxyError> {
    Sysproxy::get_system_proxy().map_err(|e| SysproxyError::GetFailed(e.to_string()))
}
