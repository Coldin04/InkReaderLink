use std::{path::PathBuf, sync::Arc};

use crate::runtime::run_on_sdk_runtime;
use crate::{SdkDeviceClient, SdkFileEntry, SdkOperationError, SdkWallpaperUploadResult};
#[uniffi::export]
impl SdkDeviceClient {
    /// Lists images managed as wallpapers by the selected firmware.
    ///
    /// # Errors
    ///
    /// Returns capability, transport, or protocol errors.
    pub async fn list_wallpapers(&self) -> Result<Vec<SdkFileEntry>, SdkOperationError> {
        let inner = Arc::clone(&self.inner);
        run_on_sdk_runtime(async move {
            inner
                .list_wallpapers()
                .await
                .map(|entries| entries.into_iter().map(Into::into).collect())
        })
        .await
    }

    /// Uploads a wallpaper image and optionally applies it to the lock screen.
    ///
    /// # Errors
    ///
    /// Returns validation, capability, file, transport, conflict, or protocol errors.
    pub async fn upload_wallpaper(
        &self,
        local_path: String,
        file_name: String,
        overwrite: bool,
        apply_to_lock_screen: bool,
    ) -> Result<SdkWallpaperUploadResult, SdkOperationError> {
        let inner = Arc::clone(&self.inner);
        run_on_sdk_runtime(async move {
            inner
                .upload_wallpaper(
                    PathBuf::from(local_path),
                    file_name,
                    overwrite,
                    apply_to_lock_screen,
                )
                .await
                .map(Into::into)
        })
        .await
    }

    /// Deletes one wallpaper image by its file name.
    ///
    /// # Errors
    ///
    /// Returns validation, capability, transport, or protocol errors.
    pub async fn delete_wallpaper(&self, file_name: String) -> Result<(), SdkOperationError> {
        let inner = Arc::clone(&self.inner);
        run_on_sdk_runtime(async move { inner.delete_wallpaper(file_name).await }).await
    }
}
