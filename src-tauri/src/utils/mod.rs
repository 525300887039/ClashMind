pub mod geoip;
pub mod path;
pub mod time;

/// Serialize any `Display` error type as its string representation for Tauri IPC.
macro_rules! impl_serialize_display {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl serde::Serialize for $ty {
                fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
                where
                    S: serde::Serializer,
                {
                    serializer.serialize_str(&self.to_string())
                }
            }
        )+
    };
}

pub(crate) use impl_serialize_display;
