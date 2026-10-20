//! Stateful `KiikoReadAdapter` protocol operations.
use crate::adapters::{DeviceAdapter, FileUpload, FontUpload, WallpaperUpload};
use crate::model::ConflictPolicy;
use crate::transport::{HttpRequest, HttpResponse, SharedTransport};
use crate::validation::join_location;
use crate::{
    DeviceInfoField, FileEntry, FileKind, FileLocation, SdkError, UploadOptions, UploadResult,
    WallpaperUploadResult, WifiCredential, WifiNetwork,
};
use async_trait::async_trait;
use std::path::PathBuf;

use super::KiikoReadAdapter;
fn ensure_kiikoread_success(response: &HttpResponse, operation: &str) -> Result<(), SdkError> {
    let body = response.text_lossy();
    match response.status {
        200..=299 => Ok(()),
        408 => Err(SdkError::Timeout),
        409 => Err(SdkError::Conflict(body)),
        413 | 507 => Err(SdkError::InsufficientStorage),
        _ => Err(SdkError::RemoteFailure(format!(
            "{operation} failed with HTTP {}: {body}",
            response.status
        ))),
    }
}
type ResponsePolicy = fn(&HttpResponse, &str) -> Result<(), SdkError>;
pub(crate) struct Session {
    pub(in crate::adapters) transport: SharedTransport,
    response_policy: ResponsePolicy,
}
impl Session {
    pub fn new(transport: SharedTransport) -> Self {
        Self::with_response_policy(transport, ensure_kiikoread_success)
    }
    pub(in crate::adapters) fn with_response_policy(
        transport: SharedTransport,
        response_policy: ResponsePolicy,
    ) -> Self {
        Self {
            transport,
            response_policy,
        }
    }
    fn ensure_success(&self, response: &HttpResponse, operation: &str) -> Result<(), SdkError> {
        (self.response_policy)(response, operation)
    }
}
impl Session {
    async fn list_kiikoread(&self, location: &FileLocation) -> Result<Vec<FileEntry>, SdkError> {
        let mut page_index = 0;
        let mut entries = Vec::new();
        loop {
            let response = self
                .transport
                .execute(KiikoReadAdapter::file_list_request(location, page_index))
                .await?;
            self.ensure_success(&response, "list files")?;
            let page = KiikoReadAdapter::parse_file_page(location, &response.text_lossy())?;
            entries.extend(page.entries);
            page_index += 1;
            if page_index >= page.pages {
                return Ok(entries);
            }
        }
    }
    async fn upload_kiikoread(
        &self,
        local_path: PathBuf,
        file_name: String,
        options: UploadOptions,
    ) -> Result<(), SdkError> {
        let overwrite = match options.conflict_policy {
            ConflictPolicy::Fail => false,
            ConflictPolicy::OverwriteWhenSupported => true,
            ConflictPolicy::ReplaceWithBackup => {
                return Err(SdkError::Unsupported(
                    "KiikoRead uses the firmware's explicit overwrite operation".to_owned(),
                ));
            }
        };
        let response = self
            .transport
            .execute(KiikoReadAdapter::book_upload_request(
                local_path, file_name, overwrite,
            ))
            .await?;
        self.ensure_success(&response, "upload")
    }
    async fn execute_success(&self, request: HttpRequest, operation: &str) -> Result<(), SdkError> {
        let response = self.transport.execute(request).await?;
        self.ensure_success(&response, operation)
    }
}
#[async_trait]
impl DeviceAdapter for Session {
    async fn verify_connection(&self) -> Result<(), SdkError> {
        let response = self
            .transport
            .execute(KiikoReadAdapter::info_request())
            .await?;
        self.ensure_success(&response, "verify device connection")?;
        KiikoReadAdapter::validate_info(&response.body)
    }
    async fn device_info(&self) -> Result<Vec<DeviceInfoField>, SdkError> {
        let response = self
            .transport
            .execute(KiikoReadAdapter::info_request())
            .await?;
        self.ensure_success(&response, "get device info")?;
        KiikoReadAdapter::parse_info(&response.body)
    }
    async fn list_files(&self, location: &FileLocation) -> Result<Vec<FileEntry>, SdkError> {
        self.list_kiikoread(location).await
    }
    async fn upload(&self, upload: FileUpload) -> Result<UploadResult, SdkError> {
        let path = join_location(&upload.location, &upload.file_name);
        self.upload_kiikoread(
            upload.local_path.clone(),
            upload.file_name.clone(),
            upload.options.clone(),
        )
        .await?;
        Ok(upload.result(path, false))
    }
    async fn delete(&self, path: &str) -> Result<(), SdkError> {
        self.execute_success(
            KiikoReadAdapter::delete_path_request(path.strip_prefix('/').unwrap_or(path))?,
            "delete",
        )
        .await
    }
    async fn list_wifi_networks(&self) -> Result<Vec<WifiNetwork>, SdkError> {
        let response = self
            .transport
            .execute(KiikoReadAdapter::info_request())
            .await?;
        self.ensure_success(&response, "list Wi-Fi networks")?;
        KiikoReadAdapter::parse_wifi_info(&response.text_lossy())
    }
    async fn save_wifi_network(&self, credential: &WifiCredential) -> Result<(), SdkError> {
        self.execute_success(
            KiikoReadAdapter::wifi_save_request(credential)?,
            "save Wi-Fi network",
        )
        .await
    }
    async fn delete_wifi_network(&self, _index: Option<u32>) -> Result<(), SdkError> {
        self.execute_success(
            KiikoReadAdapter::wifi_delete_request(),
            "delete Wi-Fi network",
        )
        .await
    }
    async fn list_wallpapers(&self) -> Result<Vec<FileEntry>, SdkError> {
        let mut page_index = 0;
        let mut entries = Vec::new();
        loop {
            let response = self
                .transport
                .execute(KiikoReadAdapter::wallpapers_list_request(page_index))
                .await?;
            self.ensure_success(&response, "list wallpapers")?;
            let page = KiikoReadAdapter::parse_wallpaper_page(&response.text_lossy())?;
            entries.extend(page.entries);
            page_index += 1;
            if page_index >= page.pages {
                return Ok(entries);
            }
        }
    }
    async fn upload_wallpaper(
        &self,
        upload: WallpaperUpload,
    ) -> Result<WallpaperUploadResult, SdkError> {
        let WallpaperUpload {
            local_path,
            file_name,
            overwrite,
            apply_to_lock_screen,
            size,
        } = upload;
        if size > 20 * 1024 * 1024 {
            return Err(SdkError::InvalidArgument(
                "wallpaper image exceeds 20 MiB".to_owned(),
            ));
        }
        if apply_to_lock_screen && size > 2 * 1024 * 1024 {
            return Err(SdkError::InvalidArgument(
                "lock-screen wallpaper image exceeds 2 MiB".to_owned(),
            ));
        }
        let response = self
            .transport
            .execute(KiikoReadAdapter::wallpaper_upload_request(
                local_path,
                file_name.clone(),
                overwrite,
                apply_to_lock_screen,
            ))
            .await?;
        let body = response.text_lossy();
        if apply_to_lock_screen
            && response.status == 500
            && body.contains("图片已上传，但设置锁屏壁纸失败")
        {
            return Err(SdkError::CommittedWithWarning(body));
        }
        self.ensure_success(&response, "upload wallpaper")?;
        Ok(WallpaperUploadResult {
            entry: FileEntry {
                name: file_name.clone(),
                path: format!("/pictures/{file_name}"),
                size,
                kind: FileKind::Other,
            },
            applied_to_lock_screen: apply_to_lock_screen,
        })
    }
    async fn delete_wallpaper(&self, file_name: &str) -> Result<(), SdkError> {
        self.execute_success(
            KiikoReadAdapter::delete_wallpaper_request(file_name)?,
            "delete wallpaper",
        )
        .await
    }
    async fn upload_font(&self, upload: FontUpload) -> Result<(), SdkError> {
        if upload.file_name.len() > 120 {
            return Err(SdkError::InvalidArgument(
                "KiikoRead font file name exceeds 120 bytes".to_owned(),
            ));
        }
        if upload.size > 32 * 1024 * 1024 {
            return Err(SdkError::InvalidArgument(
                "font file exceeds 32 MiB".to_owned(),
            ));
        }
        let response = self
            .transport
            .execute_with_upload_progress(
                KiikoReadAdapter::font_upload_request(
                    upload.local_path,
                    upload.file_name,
                    upload.overwrite,
                ),
                upload.progress,
            )
            .await?;
        self.ensure_success(&response, "upload font")
    }
}
