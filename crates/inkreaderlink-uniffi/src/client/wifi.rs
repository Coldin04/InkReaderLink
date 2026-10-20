use std::sync::Arc;

use crate::runtime::run_on_sdk_runtime;
use crate::{SdkDeviceClient, SdkOperationError, SdkWifiCredential, SdkWifiNetwork};
#[uniffi::export]
impl SdkDeviceClient {
    /// Lists saved Wi-Fi credentials without passwords.
    ///
    /// # Errors
    ///
    /// Returns capability, transport, or protocol errors.
    pub async fn list_wifi_networks(&self) -> Result<Vec<SdkWifiNetwork>, SdkOperationError> {
        let inner = Arc::clone(&self.inner);
        run_on_sdk_runtime(async move {
            inner
                .list_wifi_networks()
                .await
                .map(|networks| networks.into_iter().map(Into::into).collect())
        })
        .await
    }

    /// Adds or updates a Wi-Fi credential.
    ///
    /// # Errors
    ///
    /// Returns validation, capability, transport, or protocol errors.
    pub async fn save_wifi_network(
        &self,
        credential: SdkWifiCredential,
    ) -> Result<(), SdkOperationError> {
        let inner = Arc::clone(&self.inner);
        run_on_sdk_runtime(async move { inner.save_wifi_network(credential.into()).await }).await
    }

    /// Deletes a saved Wi-Fi credential.
    ///
    /// # Errors
    ///
    /// Returns validation, capability, transport, or protocol errors.
    pub async fn delete_wifi_network(&self, index: Option<u32>) -> Result<(), SdkOperationError> {
        let inner = Arc::clone(&self.inner);
        run_on_sdk_runtime(async move { inner.delete_wifi_network(index).await }).await
    }
}
