use std::sync::Arc;

use crate::runtime::run_on_sdk_runtime;
use crate::{SdkDeviceClient, SdkOpdsCredential, SdkOpdsServer, SdkOperationError};
#[uniffi::export]
impl SdkDeviceClient {
    /// Lists saved OPDS servers without passwords.
    /// # Errors
    /// Returns capability, transport, or protocol errors.
    pub async fn list_opds_servers(&self) -> Result<Vec<SdkOpdsServer>, SdkOperationError> {
        let inner = Arc::clone(&self.inner);
        run_on_sdk_runtime(async move {
            inner
                .list_opds_servers()
                .await
                .map(|servers| servers.into_iter().map(Into::into).collect())
        })
        .await
    }

    /// Adds or updates an OPDS server.
    /// # Errors
    /// Returns validation, capability, transport, or protocol errors.
    pub async fn save_opds_server(
        &self,
        credential: SdkOpdsCredential,
    ) -> Result<(), SdkOperationError> {
        let inner = Arc::clone(&self.inner);
        run_on_sdk_runtime(async move { inner.save_opds_server(credential.into()).await }).await
    }

    /// Deletes an OPDS server by index.
    /// # Errors
    /// Returns capability, transport, or protocol errors.
    pub async fn delete_opds_server(&self, index: u32) -> Result<(), SdkOperationError> {
        let inner = Arc::clone(&self.inner);
        run_on_sdk_runtime(async move { inner.delete_opds_server(index).await }).await
    }
}
