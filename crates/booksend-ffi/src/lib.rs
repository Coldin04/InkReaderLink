use std::sync::Arc;

use booksend_core::{Capability, DeviceKind, DeviceProfile};

#[derive(Clone, Debug, uniffi::Record)]
pub struct SdkDeviceIdentity {
    pub device_type: String,
    pub device_id: Option<String>,
    pub firmware_version: Option<String>,
    pub protocol_version: Option<u32>,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct SdkDeviceProfile {
    pub identity: SdkDeviceIdentity,
    pub capabilities: Vec<String>,
}

impl From<DeviceProfile> for SdkDeviceProfile {
    fn from(profile: DeviceProfile) -> Self {
        Self {
            identity: SdkDeviceIdentity {
                device_type: profile.identity.device_type,
                device_id: profile.identity.device_id,
                firmware_version: profile.identity.firmware_version,
                protocol_version: profile.identity.protocol_version,
            },
            capabilities: profile
                .capabilities
                .iter()
                .map(Capability::id)
                .map(str::to_owned)
                .collect(),
        }
    }
}

#[derive(Debug, uniffi::Object)]
pub struct BooksendSdk;

#[uniffi::export]
impl BooksendSdk {
    #[uniffi::constructor]
    pub fn new() -> Arc<Self> {
        Arc::new(Self)
    }

    pub fn version(&self) -> String {
        env!("CARGO_PKG_VERSION").to_owned()
    }

    pub fn baseline_profile(&self, device_type: String) -> Option<SdkDeviceProfile> {
        let kind = match device_type.as_str() {
            "read-pico" => DeviceKind::ReadPico,
            "crosspoint" => DeviceKind::CrossPoint,
            _ => return None,
        };
        Some(DeviceProfile::baseline(kind).into())
    }
}

uniffi::setup_scaffolding!();

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_extensible_string_capabilities() {
        let sdk = BooksendSdk::new();
        let profile = sdk.baseline_profile("read-pico".to_owned()).unwrap();

        assert_eq!(profile.identity.device_type, "read-pico");
        assert!(profile.capabilities.contains(&"files.upload".to_owned()));
    }
}
