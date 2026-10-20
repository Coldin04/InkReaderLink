use inkreaderlink_core::{DeviceClient, route_device};
use std::sync::Arc;

use crate::{SdkConnectionParameter, SdkDeviceInfoField, SdkDeviceProfile, SdkOperationError};
use crate::{
    connection::{resolve_builtin_device_definition, resolve_connection_parameters},
    runtime::run_on_sdk_runtime,
};
use std::time::Duration;
#[derive(Debug, uniffi::Object)]
pub struct SdkDeviceClient {
    inner: Arc<DeviceClient>,
}

mod files;
mod fonts;
mod opds;
mod settings;
mod wallpapers;
mod wifi;
#[uniffi::export]
impl SdkDeviceClient {
    /// Connects a typed device client to an HTTP origin.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown device type or invalid URL.
    #[uniffi::constructor]
    #[allow(clippy::needless_pass_by_value)]
    pub fn connect(
        device_type: String,
        base_url: String,
        timeout_ms: u64,
    ) -> Result<Arc<Self>, SdkOperationError> {
        let definition = resolve_builtin_device_definition(&device_type)?;
        let kind = route_device(&device_type).ok_or_else(|| SdkOperationError::Unsupported {
            detail: format!("unknown device type: {device_type}"),
        })?;
        let inner =
            DeviceClient::connect(kind, &base_url, Duration::from_millis(timeout_ms.max(1)))
                .map_err(SdkOperationError::from)?
                .with_device_definition(definition);
        Ok(Arc::new(Self {
            inner: Arc::new(inner),
        }))
    }

    /// Connects only after the selected device's information endpoint is verified.
    ///
    /// # Errors
    ///
    /// Returns an error when the URL is invalid, the device cannot be reached, or its
    /// information response does not match the selected device type.
    #[uniffi::constructor]
    #[allow(clippy::needless_pass_by_value)]
    pub async fn connect_and_verify(
        device_type: String,
        base_url: String,
        timeout_ms: u64,
    ) -> Result<Arc<Self>, SdkOperationError> {
        let client = Self::connect(device_type, base_url, timeout_ms)?;
        let inner = Arc::clone(&client.inner);
        run_on_sdk_runtime(async move { inner.verify_connection().await }).await?;
        Ok(client)
    }

    /// Connects after validating the typed fields declared for the selected device.
    ///
    /// # Errors
    ///
    /// Returns an error for unknown, duplicate, missing, or mistyped fields, or when
    /// the device address cannot be reached or verified.
    #[uniffi::constructor]
    #[allow(clippy::needless_pass_by_value)]
    pub async fn connect_and_verify_with_parameters(
        device_type: String,
        parameters: Vec<SdkConnectionParameter>,
        timeout_ms: u64,
    ) -> Result<Arc<Self>, SdkOperationError> {
        let definition = resolve_builtin_device_definition(&device_type)?;
        let resolved = resolve_connection_parameters(&definition, parameters)?;
        let kind = route_device(&device_type).ok_or_else(|| SdkOperationError::Unsupported {
            detail: format!("unknown device type: {device_type}"),
        })?;
        let inner = DeviceClient::connect(
            kind,
            &resolved.address,
            Duration::from_millis(timeout_ms.max(1)),
        )
        .map_err(SdkOperationError::from)?
        .with_device_definition(definition)
        .with_session_credentials(resolved.session_credentials);
        let inner = Arc::new(inner);
        let client = Arc::new(Self {
            inner: Arc::clone(&inner),
        });
        run_on_sdk_runtime(async move { inner.verify_connection().await }).await?;
        Ok(client)
    }

    #[must_use]
    pub fn profile(&self) -> SdkDeviceProfile {
        self.inner.profile().clone().into()
    }

    /// Fetches device information for an information page. Keys are stable identifiers
    /// intended for app-side localization; values are returned as displayable text.
    ///
    /// # Errors
    ///
    /// Returns the mapped SDK operation error for unsupported capabilities,
    /// transport failures, or invalid device responses.
    pub async fn device_info(&self) -> Result<Vec<SdkDeviceInfoField>, SdkOperationError> {
        let inner = Arc::clone(&self.inner);
        run_on_sdk_runtime(async move {
            inner
                .device_info()
                .await
                .map(|fields| fields.into_iter().map(Into::into).collect())
        })
        .await
    }
}
