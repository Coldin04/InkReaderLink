use inkreaderlink_core::UploadProgressSink;
use std::{path::PathBuf, sync::Arc};

use crate::{
    SdkDeviceClient, SdkFileDownload, SdkFileEntry, SdkFileLocation, SdkOperationError,
    SdkUploadOptions, SdkUploadProgressObserver, SdkUploadResult,
};
use crate::{progress::FfiProgressSink, runtime::run_on_sdk_runtime};
#[uniffi::export]
impl SdkDeviceClient {
    /// Lists files at a device location.
    ///
    /// # Errors
    ///
    /// Returns capability, transport, or protocol errors.
    pub async fn list_files(
        &self,
        location: SdkFileLocation,
    ) -> Result<Vec<SdkFileEntry>, SdkOperationError> {
        let inner = Arc::clone(&self.inner);
        run_on_sdk_runtime(async move {
            inner
                .list_files(location.into())
                .await
                .map(|entries| entries.into_iter().map(Into::into).collect())
        })
        .await
    }

    /// Streams a local file to the device.
    ///
    /// # Errors
    ///
    /// Returns validation, file, capability, transport, or protocol errors.
    pub async fn upload(
        &self,
        local_path: String,
        file_name: String,
        location: SdkFileLocation,
        options: SdkUploadOptions,
        progress_observer: Option<Arc<dyn SdkUploadProgressObserver>>,
    ) -> Result<SdkUploadResult, SdkOperationError> {
        let inner = Arc::clone(&self.inner);
        run_on_sdk_runtime(async move {
            inner
                .upload(
                    PathBuf::from(local_path),
                    file_name,
                    location.into(),
                    options.into(),
                    progress_observer.map(|observer| {
                        Arc::new(FfiProgressSink { observer }) as Arc<dyn UploadProgressSink>
                    }),
                )
                .await
                .map(Into::into)
        })
        .await
    }

    /// Deletes a remote file or empty directory.
    ///
    /// # Errors
    ///
    /// Returns validation, capability, transport, or protocol errors.
    pub async fn delete(&self, path: String) -> Result<(), SdkOperationError> {
        let inner = Arc::clone(&self.inner);
        run_on_sdk_runtime(async move { inner.delete(path).await }).await
    }

    /// Deletes multiple remote paths. Supported-device endpoint differences are
    /// handled by the SDK; sequential execution stops at the first failure.
    ///
    /// # Errors
    ///
    /// Returns validation, capability, transport, or protocol errors.
    pub async fn delete_files(&self, paths: Vec<String>) -> Result<(), SdkOperationError> {
        let inner = Arc::clone(&self.inner);
        run_on_sdk_runtime(async move { inner.delete_files(paths).await }).await
    }

    /// Streams a remote file to a local destination.
    ///
    /// # Errors
    ///
    /// Returns file, capability, transport, or protocol errors.
    pub async fn download(
        &self,
        path: String,
        destination: String,
    ) -> Result<(), SdkOperationError> {
        let inner = Arc::clone(&self.inner);
        run_on_sdk_runtime(async move { inner.download(path, PathBuf::from(destination)).await })
            .await
    }

    /// Downloads multiple remote files to the supplied local destinations.
    /// Downloads are streamed one at a time and stop at the first failure.
    ///
    /// # Errors
    ///
    /// Returns validation, file, capability, transport, or protocol errors.
    pub async fn download_files(
        &self,
        files: Vec<SdkFileDownload>,
    ) -> Result<(), SdkOperationError> {
        let inner = Arc::clone(&self.inner);
        let files = files.into_iter().map(Into::into).collect();
        run_on_sdk_runtime(async move { inner.download_files(files).await }).await
    }

    /// Creates a remote directory.
    ///
    /// # Errors
    ///
    /// Returns validation, capability, transport, or protocol errors.
    pub async fn create_directory(
        &self,
        parent: String,
        name: String,
    ) -> Result<(), SdkOperationError> {
        let inner = Arc::clone(&self.inner);
        run_on_sdk_runtime(async move { inner.create_directory(parent, name).await }).await
    }

    /// Renames a remote file.
    ///
    /// # Errors
    ///
    /// Returns validation, capability, transport, or protocol errors.
    pub async fn rename(&self, path: String, new_name: String) -> Result<(), SdkOperationError> {
        let inner = Arc::clone(&self.inner);
        run_on_sdk_runtime(async move { inner.rename(path, new_name).await }).await
    }

    /// Moves a remote file.
    ///
    /// # Errors
    ///
    /// Returns validation, capability, transport, or protocol errors.
    pub async fn move_file(
        &self,
        path: String,
        destination: String,
    ) -> Result<(), SdkOperationError> {
        let inner = Arc::clone(&self.inner);
        run_on_sdk_runtime(async move { inner.move_file(path, destination).await }).await
    }

    /// Moves multiple remote files into one existing directory, serially.
    ///
    /// # Errors
    ///
    /// Returns validation, capability, transport, or protocol errors.
    pub async fn move_files(
        &self,
        paths: Vec<String>,
        destination: String,
    ) -> Result<(), SdkOperationError> {
        let inner = Arc::clone(&self.inner);
        run_on_sdk_runtime(async move { inner.move_files(paths, destination).await }).await
    }
}
