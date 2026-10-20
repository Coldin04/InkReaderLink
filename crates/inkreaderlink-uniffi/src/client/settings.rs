use std::sync::Arc;

use crate::runtime::run_on_sdk_runtime;
use crate::{SdkDeviceClient, SdkOperationError, SdkSettingChange, SdkSettingsSnapshot};
#[uniffi::export]
impl SdkDeviceClient {
    /// Returns current editable settings and rendering metadata.
    /// # Errors
    /// Returns capability, transport, or protocol errors.
    pub async fn list_settings(&self) -> Result<SdkSettingsSnapshot, SdkOperationError> {
        let inner = Arc::clone(&self.inner);
        run_on_sdk_runtime(async move { inner.list_settings().await.map(Into::into) }).await
    }

    /// Validates and applies changes against a previously returned snapshot.
    /// # Errors
    /// Returns capability, conflict, validation, transport, or protocol errors.
    pub async fn apply_settings(
        &self,
        expected: SdkSettingsSnapshot,
        changes: Vec<SdkSettingChange>,
    ) -> Result<SdkSettingsSnapshot, SdkOperationError> {
        let inner = Arc::clone(&self.inner);
        run_on_sdk_runtime(async move {
            inner
                .apply_settings(
                    expected.into(),
                    changes.into_iter().map(Into::into).collect(),
                )
                .await
                .map(Into::into)
        })
        .await
    }
}
