//! `AuraReader` web configuration and raw-file transfer protocol.
pub mod definition;
pub mod session;

#[cfg(test)]
mod session_tests;

use serde::Deserialize;

use crate::{DeviceInfoField, SdkError};

#[derive(Clone, Debug, Deserialize)]
pub(crate) struct Info {
    pub mode: String,
    pub free_bytes: u64,
    pub is_flash: bool,
    #[serde(default)]
    pub file_limit: Option<u64>,
    #[serde(default)]
    pub wifi_configured: bool,
    #[serde(default)]
    pub wifi_ssid: Option<String>,
    #[serde(default)]
    pub fonts_enabled: bool,
    #[serde(default)]
    pub font_root: Option<String>,
    #[serde(default)]
    pub font_limit: Option<u64>,
    #[serde(default)]
    #[serde(alias = "root")]
    pub storage_root: Option<String>,
    #[serde(default)]
    pub rtc_synced: Option<bool>,
    #[serde(default)]
    pub device_unix: Option<u64>,
    #[serde(default)]
    pub lock_dir: Option<String>,
    #[serde(default)]
    pub book_px: Option<i64>,
    #[serde(default)]
    pub book_bold: Option<bool>,
    #[serde(default)]
    pub reader_heading_right: Option<bool>,
    #[serde(default)]
    pub reader_margin: Option<i64>,
    #[serde(default)]
    pub reader_line_permille: Option<i64>,
    #[serde(default)]
    pub reader_underline: Option<i64>,
    #[serde(default)]
    pub reader_wave: Option<i64>,
    #[serde(default)]
    pub reader_full_every: Option<i64>,
    #[serde(default)]
    pub reader_indent: Option<bool>,
    #[serde(default)]
    pub reader_justify: Option<bool>,
    #[serde(default)]
    pub reader_chapter_opt: Option<bool>,
    #[serde(default)]
    pub reader_blank_opt: Option<bool>,
}

impl Info {
    pub(crate) fn parse(bytes: &[u8]) -> Result<Self, SdkError> {
        serde_json::from_slice(bytes)
            .map_err(|e| SdkError::RemoteFailure(format!("invalid AuraReader /info response: {e}")))
    }

    pub(crate) fn fields(&self) -> Vec<DeviceInfoField> {
        let mut fields = vec![
            field("network_mode", self.mode.clone()),
            field("storage_free_bytes", self.free_bytes),
            field("storage_is_flash", self.is_flash),
            field("wifi_configured", self.wifi_configured),
            field("fonts_enabled", self.fonts_enabled),
        ];
        for (key, value) in [
            ("storage_file_limit", self.file_limit),
            ("font_file_limit", self.font_limit),
            ("device_unix", self.device_unix),
        ] {
            if let Some(value) = value {
                fields.push(field(key, value));
            }
        }
        for (key, value) in [
            ("storage_root", &self.storage_root),
            ("font_root", &self.font_root),
            ("wifi_ssid", &self.wifi_ssid),
            ("lock_dir", &self.lock_dir),
        ] {
            if let Some(value) = value {
                fields.push(field(key, value.clone()));
            }
        }
        if let Some(value) = self.rtc_synced {
            fields.push(field("rtc_synced", value));
        }
        fields
    }
}

#[allow(clippy::needless_pass_by_value)]
pub(crate) fn field(key: &str, value: impl ToString) -> DeviceInfoField {
    DeviceInfoField {
        key: key.to_owned(),
        value: value.to_string(),
    }
}
