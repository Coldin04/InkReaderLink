use std::{path::PathBuf, sync::Arc};

use crate::{SdkDeviceClient, SdkFontCatalog, SdkOperationError, SdkUploadProgressObserver};
use crate::{progress::FfiProgressSink, runtime::run_on_sdk_runtime};
#[uniffi::export]
impl SdkDeviceClient {
    /// Lists installed SD-card font families.
    /// # Errors
    /// Returns capability, transport, or protocol errors.
    pub async fn list_fonts(&self) -> Result<SdkFontCatalog, SdkOperationError> {
        let inner = Arc::clone(&self.inner);
        run_on_sdk_runtime(async move { inner.list_fonts().await.map(Into::into) }).await
    }

    /// Streams a device-supported font file. Read Pico rejects same-name files by default.
    /// # Errors
    /// Returns validation, capability, file, transport, or protocol errors.
    pub async fn upload_font(
        &self,
        family: String,
        local_path: String,
        file_name: String,
    ) -> Result<(), SdkOperationError> {
        let inner = Arc::clone(&self.inner);
        run_on_sdk_runtime(async move {
            inner
                .upload_font(family, PathBuf::from(local_path), file_name)
                .await
        })
        .await
    }

    /// Streams a font file and reports bytes supplied to the HTTP request body.
    /// # Errors
    /// Returns validation, capability, file, transport, or protocol errors.
    pub async fn upload_font_with_progress(
        &self,
        family: String,
        local_path: String,
        file_name: String,
        progress_observer: Arc<dyn SdkUploadProgressObserver>,
    ) -> Result<(), SdkOperationError> {
        let inner = Arc::clone(&self.inner);
        run_on_sdk_runtime(async move {
            inner
                .upload_font_with_progress(
                    family,
                    PathBuf::from(local_path),
                    file_name,
                    Arc::new(FfiProgressSink {
                        observer: progress_observer,
                    }),
                )
                .await
        })
        .await
    }

    /// Streams a font file with progress and an explicit overwrite choice.
    /// # Errors
    /// Returns validation, capability, file, transport, or protocol errors.
    pub async fn upload_font_with_overwrite_and_progress(
        &self,
        family: String,
        local_path: String,
        file_name: String,
        overwrite: bool,
        progress_observer: Arc<dyn SdkUploadProgressObserver>,
    ) -> Result<(), SdkOperationError> {
        let inner = Arc::clone(&self.inner);
        run_on_sdk_runtime(async move {
            inner
                .upload_font_with_overwrite_and_progress(
                    family,
                    PathBuf::from(local_path),
                    file_name,
                    overwrite,
                    Some(Arc::new(FfiProgressSink {
                        observer: progress_observer,
                    })),
                )
                .await
        })
        .await
    }

    /// Streams a font and optionally replaces a same-name Read Pico font.
    /// # Errors
    /// Returns validation, capability, file, transport, or protocol errors.
    pub async fn upload_font_with_overwrite(
        &self,
        family: String,
        local_path: String,
        file_name: String,
        overwrite: bool,
    ) -> Result<(), SdkOperationError> {
        let inner = Arc::clone(&self.inner);
        run_on_sdk_runtime(async move {
            inner
                .upload_font_with_overwrite(family, PathBuf::from(local_path), file_name, overwrite)
                .await
        })
        .await
    }

    /// Deletes an installed font family.
    /// # Errors
    /// Returns validation, capability, transport, or protocol errors.
    pub async fn delete_font_family(&self, family: String) -> Result<(), SdkOperationError> {
        let inner = Arc::clone(&self.inner);
        run_on_sdk_runtime(async move { inner.delete_font_family(family).await }).await
    }
}
