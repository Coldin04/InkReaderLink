use crate::{SdkDeviceProfile, SdkSupportedDevice, connection::resolve_builtin_device_definition};
use inkreaderlink_core::{DeviceProfile, built_in_definitions};
use std::sync::Arc;
#[derive(Debug, uniffi::Object)]
pub struct BooksendSdk;

#[uniffi::export]
impl BooksendSdk {
    #[uniffi::constructor]
    #[must_use]
    pub fn new() -> Arc<Self> {
        Arc::new(Self)
    }

    #[must_use]
    pub fn version(&self) -> String {
        env!("CARGO_PKG_VERSION").to_owned()
    }

    /// Lists SDK-supported devices and the fields required to configure each connection.
    #[must_use]
    pub fn supported_devices(&self) -> Vec<SdkSupportedDevice> {
        built_in_definitions().into_iter().map(Into::into).collect()
    }

    #[must_use]
    #[allow(clippy::needless_pass_by_value)]
    pub fn baseline_profile(&self, device_type: String) -> Option<SdkDeviceProfile> {
        let definition = resolve_builtin_device_definition(&device_type).ok()?;
        Some(DeviceProfile::from_definition(definition).into())
    }
}
