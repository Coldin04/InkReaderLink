//! Stateful `ReadPicoAdapter` protocol operations.
use crate::adapters::{DeviceAdapter, FileUpload, FontUpload};
use crate::model::ConflictPolicy;
use crate::transport::{HttpRequest, HttpResponse, SharedTransport};
use crate::validation::{join_location, validate_file_name};
use crate::{
    DeviceInfoField, FileEntry, FileLocation, SdkError, UploadOptions, UploadResult,
    WifiCredential, WifiNetwork,
};
use async_trait::async_trait;
use std::path::PathBuf;

use super::ReadPicoAdapter;
fn ensure_success(response: &HttpResponse, operation: &str) -> Result<(), SdkError> {
    if (200..300).contains(&response.status) {
        return Ok(());
    }
    let message = response.text_lossy();
    match response.status {
        408 => Err(SdkError::Timeout),
        409 => Err(SdkError::Conflict(message)),
        413 | 507 => Err(SdkError::InsufficientStorage),
        400 if message.to_ascii_lowercase().contains("exist") => Err(SdkError::Conflict(message)),
        _ => Err(SdkError::RemoteFailure(format!(
            "{operation} failed with HTTP {}: {message}",
            response.status
        ))),
    }
}
fn ensure_read_pico_success(response: &HttpResponse, operation: &str) -> Result<(), SdkError> {
    let body = response.text_lossy();
    let value = serde_json::from_slice::<serde_json::Value>(&response.body).ok();
    let committed = value.as_ref().is_some_and(|item| {
        item.get("committed").and_then(serde_json::Value::as_bool) == Some(true)
            || item.get("deleted").and_then(serde_json::Value::as_bool) == Some(true)
    });
    let cleanup_failed = value.as_ref().is_some_and(|item| {
        item.get("progress_cleanup_failed")
            .and_then(serde_json::Value::as_bool)
            == Some(true)
    });
    if committed && cleanup_failed {
        return Err(SdkError::CommittedWithWarning(body));
    }
    ensure_success(response, operation)?;
    if value
        .as_ref()
        .is_some_and(|item| item.get("warning").is_some())
    {
        return Err(SdkError::CommittedWithWarning(body));
    }
    Ok(())
}
pub(in crate::adapters) fn read_pico_name(path: &str) -> Result<&str, SdkError> {
    let name = path.strip_prefix('/').unwrap_or(path);
    validate_file_name(name)?;
    Ok(name)
}
pub(crate) struct Session {
    transport: SharedTransport,
}
impl Session {
    pub fn new(transport: SharedTransport) -> Self {
        Self { transport }
    }
}
impl Session {
    async fn list_read_pico(&self, location: FileLocation) -> Result<Vec<FileEntry>, SdkError> {
        if !matches!(location, FileLocation::Root) {
            return Err(SdkError::Unsupported(
                "Read Pico does not support directory selection".to_owned(),
            ));
        }
        let mut page_index = 0;
        let mut entries = Vec::new();
        loop {
            let response = self
                .transport
                .execute(ReadPicoAdapter::list_request(page_index))
                .await?;
            ensure_success(&response, "list files")?;
            let page = ReadPicoAdapter::parse_file_page(&location, &response.text_lossy())?;
            entries.extend(page.entries);
            page_index += 1;
            if page_index >= page.pages {
                return Ok(entries);
            }
        }
    }
    async fn upload_read_pico(
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
                    "Read Pico handles overwrite atomically and does not expose backup replacement"
                        .to_owned(),
                ));
            }
        };
        let response = self
            .transport
            .execute(ReadPicoAdapter::upload_request(
                local_path,
                file_name,
                overwrite,
                options.content_type,
            ))
            .await?;
        ensure_read_pico_success(&response, "upload")
    }
    async fn ensure_read_pico_font_is_missing(&self, file_name: &str) -> Result<(), SdkError> {
        // The firmware closes a rejected PUT before consuming its body. Probe
        // first so duplicate names produce a reliable conflict response.
        let existing = self
            .transport
            .execute(ReadPicoAdapter::font_info_request(file_name.to_owned()))
            .await?;
        match existing.status {
            200 => Err(SdkError::Conflict(format!(
                "font already exists: {file_name}"
            ))),
            404 => Ok(()),
            409 => Err(SdkError::Unsupported(
                "Read Pico font upload requires a mounted TF card".to_owned(),
            )),
            _ => ensure_read_pico_success(&existing, "check font before upload"),
        }
    }
    async fn execute_success(&self, request: HttpRequest, operation: &str) -> Result<(), SdkError> {
        let response = self.transport.execute(request).await?;
        ensure_read_pico_success(&response, operation)
    }
}
#[async_trait]
impl DeviceAdapter for Session {
    async fn verify_connection(&self) -> Result<(), SdkError> {
        let response = self
            .transport
            .execute(ReadPicoAdapter::info_request())
            .await?;
        ensure_success(&response, "verify device connection")?;
        ReadPicoAdapter::validate_info(&response.body)
    }
    async fn device_info(&self) -> Result<Vec<DeviceInfoField>, SdkError> {
        let response = self
            .transport
            .execute(ReadPicoAdapter::info_request())
            .await?;
        ensure_success(&response, "get device info")?;
        ReadPicoAdapter::parse_info(&response.body)
    }
    async fn list_files(&self, location: &FileLocation) -> Result<Vec<FileEntry>, SdkError> {
        self.list_read_pico(location.clone()).await
    }
    async fn upload(&self, upload: FileUpload) -> Result<UploadResult, SdkError> {
        let path = join_location(&upload.location, &upload.file_name);
        self.upload_read_pico(
            upload.local_path.clone(),
            upload.file_name.clone(),
            upload.options.clone(),
        )
        .await?;
        Ok(upload.result(path, false))
    }
    fn validate_delete(&self, path: &str) -> Result<(), SdkError> {
        read_pico_name(path).map(|_| ())
    }
    async fn delete(&self, path: &str) -> Result<(), SdkError> {
        self.execute_success(
            ReadPicoAdapter::delete_request(read_pico_name(path)?),
            "delete",
        )
        .await
    }
    async fn list_wifi_networks(&self) -> Result<Vec<WifiNetwork>, SdkError> {
        let response = self
            .transport
            .execute(ReadPicoAdapter::info_request())
            .await?;
        ensure_success(&response, "list Wi-Fi networks")?;
        ReadPicoAdapter::parse_wifi_info(&response.text_lossy())
    }
    async fn save_wifi_network(&self, credential: &WifiCredential) -> Result<(), SdkError> {
        self.execute_success(
            ReadPicoAdapter::wifi_save_request(credential)?,
            "save Wi-Fi network",
        )
        .await
    }
    async fn delete_wifi_network(&self, _index: Option<u32>) -> Result<(), SdkError> {
        self.execute_success(
            ReadPicoAdapter::wifi_delete_request(),
            "delete Wi-Fi network",
        )
        .await
    }
    async fn upload_font(&self, upload: FontUpload) -> Result<(), SdkError> {
        if upload.file_name.len() > 120 {
            return Err(SdkError::InvalidArgument(
                "Read Pico font file name exceeds 120 bytes".to_owned(),
            ));
        }
        if upload.size > 32 * 1024 * 1024 {
            return Err(SdkError::InvalidArgument(
                "font file exceeds 32 MiB".to_owned(),
            ));
        }
        if !upload.overwrite {
            self.ensure_read_pico_font_is_missing(&upload.file_name)
                .await?;
        }
        let response = self
            .transport
            .execute_with_upload_progress(
                ReadPicoAdapter::font_upload_request(
                    upload.local_path,
                    upload.file_name,
                    upload.overwrite,
                ),
                upload.progress,
            )
            .await?;
        if response.status == 409 {
            let error = serde_json::from_slice::<serde_json::Value>(&response.body).ok();
            if error
                .as_ref()
                .and_then(|v| v.get("conflict"))
                .and_then(serde_json::Value::as_bool)
                != Some(true)
            {
                return Err(SdkError::Unsupported(
                    "Read Pico font upload requires a mounted TF card".to_owned(),
                ));
            }
        }
        ensure_read_pico_success(&response, "upload font")
    }
}
