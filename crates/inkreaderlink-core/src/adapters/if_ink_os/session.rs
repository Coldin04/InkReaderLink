//! Stateful `IfInkOsAdapter` protocol operations.
use crate::adapters::{DeviceAdapter, FileUpload, FontUpload, WallpaperUpload};
use crate::model::ConflictPolicy;
use crate::transport::{HttpRequest, SharedTransport};
use crate::validation::{join_location, validate_remote_path};
use crate::{
    DeviceInfoField, FileEntry, FileLocation, SdkError, UploadOptions, UploadResult,
    WallpaperUploadResult, WifiCredential, WifiNetwork,
};
use async_trait::async_trait;
use std::path::{Path, PathBuf};

use super::IfInkOsAdapter;
use crate::adapters::kiikoread;
pub(crate) struct Session {
    base: kiikoread::session::Session,
}
impl Session {
    pub fn new(transport: SharedTransport) -> Self {
        Self {
            base: kiikoread::session::Session::with_response_policy(
                transport,
                IfInkOsAdapter::ensure_success,
            ),
        }
    }
}
impl Session {
    async fn upload_if_ink_os(
        &self,
        local_path: PathBuf,
        file_name: String,
        location: &FileLocation,
        options: UploadOptions,
        size: u64,
    ) -> Result<(), SdkError> {
        let overwrite = match options.conflict_policy {
            ConflictPolicy::Fail => false,
            ConflictPolicy::OverwriteWhenSupported => true,
            ConflictPolicy::ReplaceWithBackup => {
                return Err(SdkError::Unsupported(
                    "if-ink OS uses the firmware's explicit overwrite operation".to_owned(),
                ));
            }
        };

        let directory = match location {
            FileLocation::Root => "",
            FileLocation::Directory(path) => path.trim_matches('/'),
        };
        let extension = Path::new(&file_name)
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default();
        let request = if directory.eq_ignore_ascii_case("markdown") {
            if directory != "markdown" {
                return Err(SdkError::Unsupported(
                    "if-ink OS imports Markdown documents only in /markdown".to_owned(),
                ));
            }
            if !extension.eq_ignore_ascii_case("md") {
                return Err(SdkError::Unsupported(
                    "if-ink OS manages /markdown through Markdown document imports".to_owned(),
                ));
            }
            if size > 256 * 1024 {
                return Err(SdkError::InvalidArgument(
                    "Markdown document exceeds 256 KiB".to_owned(),
                ));
            }
            IfInkOsAdapter::document_upload_request(local_path, file_name, overwrite)
        } else if directory
            .get(.."markdown".len())
            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("markdown"))
            && directory
                .get("markdown".len()..)
                .is_some_and(|suffix| suffix.starts_with('/'))
        {
            return Err(SdkError::Unsupported(
                "if-ink OS imports Markdown documents only in /markdown".to_owned(),
            ));
        } else {
            let target = join_location(location, &file_name);
            validate_remote_path(&target)?;
            IfInkOsAdapter::file_upload_request(
                local_path,
                target.trim_start_matches('/').to_owned(),
                overwrite,
            )
        };
        let response = self.base.transport.execute(request).await?;
        IfInkOsAdapter::ensure_success(&response, "upload")
    }
    async fn execute_success(&self, request: HttpRequest, operation: &str) -> Result<(), SdkError> {
        let response = self.base.transport.execute(request).await?;
        IfInkOsAdapter::ensure_success(&response, operation)
    }
}
#[async_trait]
impl DeviceAdapter for Session {
    async fn verify_connection(&self) -> Result<(), SdkError> {
        self.base.verify_connection().await
    }
    async fn device_info(&self) -> Result<Vec<DeviceInfoField>, SdkError> {
        self.base.device_info().await
    }
    async fn list_files(&self, location: &FileLocation) -> Result<Vec<FileEntry>, SdkError> {
        self.base.list_files(location).await
    }
    async fn delete(&self, path: &str) -> Result<(), SdkError> {
        self.base.delete(path).await
    }
    async fn list_wifi_networks(&self) -> Result<Vec<WifiNetwork>, SdkError> {
        self.base.list_wifi_networks().await
    }
    async fn save_wifi_network(&self, credential: &WifiCredential) -> Result<(), SdkError> {
        self.base.save_wifi_network(credential).await
    }
    async fn list_wallpapers(&self) -> Result<Vec<FileEntry>, SdkError> {
        self.base.list_wallpapers().await
    }
    async fn upload_wallpaper(
        &self,
        upload: WallpaperUpload,
    ) -> Result<WallpaperUploadResult, SdkError> {
        self.base.upload_wallpaper(upload).await
    }
    async fn delete_wallpaper(&self, file_name: &str) -> Result<(), SdkError> {
        self.base.delete_wallpaper(file_name).await
    }
    async fn upload_font(&self, upload: FontUpload) -> Result<(), SdkError> {
        self.base.upload_font(upload).await
    }
    async fn upload(&self, upload: FileUpload) -> Result<UploadResult, SdkError> {
        let path = join_location(&upload.location, &upload.file_name);
        self.upload_if_ink_os(
            upload.local_path.clone(),
            upload.file_name.clone(),
            &upload.location,
            upload.options.clone(),
            upload.size,
        )
        .await?;
        Ok(upload.result(path, false))
    }
    async fn create_directory(&self, parent: &str, name: &str) -> Result<(), SdkError> {
        self.execute_success(
            IfInkOsAdapter::create_directory_request(parent, name)?,
            "create directory",
        )
        .await
    }
    async fn rename(&self, path: &str, new_name: &str) -> Result<(), SdkError> {
        self.execute_success(IfInkOsAdapter::rename_request(path, new_name)?, "rename")
            .await
    }
    async fn move_file(&self, path: &str, destination: &str) -> Result<(), SdkError> {
        self.execute_success(IfInkOsAdapter::move_request(path, destination)?, "move")
            .await
    }
    async fn delete_wifi_network(&self, _index: Option<u32>) -> Result<(), SdkError> {
        self.execute_success(
            IfInkOsAdapter::wifi_delete_request(),
            "delete Wi-Fi network",
        )
        .await
    }
}
