//! Stateful `CrossPointAdapter` protocol operations.
use crate::adapters::{DeviceAdapter, FileUpload, FontUpload};
use crate::model::ConflictPolicy;
use crate::transport::{HttpRequest, HttpResponse, SharedTransport};
use crate::validation::{join_location, location_string, validate_file_name};
use crate::{
    DeviceInfoField, FileEntry, FileLocation, FontCatalog, OpdsCredential, OpdsServer, SdkError,
    SettingChange, SettingsSnapshot, SettingsValidationPolicy, UploadOptions, UploadProgressSink,
    UploadResult, WifiCredential, WifiNetwork,
};
use async_trait::async_trait;
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use super::CrossPointAdapter;
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
fn validate_font_family(family: &str) -> Result<(), SdkError> {
    validate_file_name(family)?;
    if family.trim() != family || family.starts_with('.') {
        return Err(SdkError::InvalidArgument(
            "invalid font family name".to_owned(),
        ));
    }
    Ok(())
}
use super::settings::{encode_changes, parse_settings};
pub(crate) struct Session {
    transport: SharedTransport,
}
impl Session {
    pub fn new(transport: SharedTransport) -> Self {
        Self { transport }
    }
}
impl Session {
    async fn list_crosspoint(&self, location: &FileLocation) -> Result<Vec<FileEntry>, SdkError> {
        let response = self
            .transport
            .execute(CrossPointAdapter::list_request(location))
            .await?;
        ensure_success(&response, "list files")?;
        Ok(CrossPointAdapter::parse_file_list(location, &response.text_lossy())?.entries)
    }
    async fn upload_crosspoint(
        &self,
        local_path: &Path,
        file_name: &str,
        location: &FileLocation,
        options: &UploadOptions,
        progress: Option<Arc<dyn UploadProgressSink>>,
    ) -> Result<bool, SdkError> {
        match options.conflict_policy {
            ConflictPolicy::OverwriteWhenSupported => {
                return Err(SdkError::Unsupported(
                    "CrossPoint does not reliably support direct overwrite".to_owned(),
                ));
            }
            ConflictPolicy::ReplaceWithBackup => {
                return self
                    .crosspoint_backup_replace(local_path, file_name, location, options, progress)
                    .await;
            }
            ConflictPolicy::Fail => {
                if self
                    .list_crosspoint(location)
                    .await?
                    .iter()
                    .any(|entry| entry.name == file_name)
                {
                    return Err(SdkError::Conflict(format!(
                        "remote file already exists: {file_name}"
                    )));
                }
            }
        }

        self.send_crosspoint_upload(local_path, file_name, location, options, progress)
            .await
    }
    async fn send_crosspoint_upload(
        &self,
        local_path: &Path,
        file_name: &str,
        location: &FileLocation,
        options: &UploadOptions,
        progress: Option<Arc<dyn UploadProgressSink>>,
    ) -> Result<bool, SdkError> {
        if options.prefer_websocket {
            super::websocket::upload(
                &self.transport,
                local_path,
                file_name,
                &location_string(location),
                progress,
            )
            .await?;
            Ok(true)
        } else {
            let response = self
                .transport
                .execute(CrossPointAdapter::upload_request(
                    location,
                    local_path.to_path_buf(),
                    file_name.to_owned(),
                    options.content_type.clone(),
                ))
                .await?;
            ensure_success(&response, "upload")?;
            Ok(false)
        }
    }
    async fn crosspoint_backup_replace(
        &self,
        local_path: &Path,
        file_name: &str,
        location: &FileLocation,
        options: &UploadOptions,
        progress: Option<Arc<dyn UploadProgressSink>>,
    ) -> Result<bool, SdkError> {
        let expected_size = tokio::fs::metadata(local_path)
            .await
            .map_err(|error| SdkError::InvalidArgument(format!("cannot verify upload: {error}")))?
            .len();
        let entries = self.list_crosspoint(location).await?;
        let old_exists = entries.iter().any(|entry| entry.name == file_name);
        if !old_exists {
            return self
                .send_crosspoint_upload(local_path, file_name, location, options, progress)
                .await;
        }

        let backup_name = format!("{file_name}.back");
        if entries.iter().any(|entry| entry.name == backup_name) {
            return Err(SdkError::Conflict(format!(
                "backup already exists: {backup_name}"
            )));
        }
        let original_path = join_location(location, file_name);
        let backup_path = join_location(location, &backup_name);
        self.execute_success(
            CrossPointAdapter::rename_request(&original_path, &backup_name),
            "create backup",
        )
        .await?;

        let used_websocket = match self
            .send_crosspoint_upload(local_path, file_name, location, options, progress)
            .await
        {
            Ok(used_websocket) => used_websocket,
            Err(upload_error) => {
                return match self
                    .execute_success(
                        CrossPointAdapter::rename_request(&backup_path, file_name),
                        "restore backup",
                    )
                    .await
                {
                    Ok(()) => Err(upload_error),
                    Err(restore_error) => Err(SdkError::RecoveryFailed(format!(
                        "upload failed ({upload_error}); restoring {backup_path} failed ({restore_error})"
                    ))),
                };
            }
        };

        let verification = self.list_crosspoint(location).await.map(|entries| {
            entries
                .iter()
                .any(|entry| entry.name == file_name && entry.size == expected_size)
        });
        if !matches!(&verification, Ok(true)) {
            let _ = self
                .execute_success(
                    CrossPointAdapter::delete_request(&original_path),
                    "remove unverified upload",
                )
                .await;
            return match self
                .execute_success(
                    CrossPointAdapter::rename_request(&backup_path, file_name),
                    "restore backup",
                )
                .await
            {
                Ok(()) => Err(verification.err().unwrap_or_else(|| {
                    SdkError::RemoteFailure(
                        "uploaded file size did not match; original restored".to_owned(),
                    )
                })),
                Err(error) => Err(SdkError::RecoveryFailed(format!(
                    "uploaded file could not be verified and restore failed: {error}"
                ))),
            };
        }

        self.execute_success(
            CrossPointAdapter::delete_request(&backup_path),
            "delete backup",
        )
        .await
        .map_err(|error| {
            SdkError::CommittedButCleanupFailed(format!(
                "new file committed but {backup_path} could not be deleted: {error}"
            ))
        })?;
        Ok(used_websocket)
    }
    async fn execute_success(&self, request: HttpRequest, operation: &str) -> Result<(), SdkError> {
        let response = self.transport.execute(request).await?;
        ensure_success(&response, operation)
    }
}
#[async_trait]
impl DeviceAdapter for Session {
    async fn verify_connection(&self) -> Result<(), SdkError> {
        let response = self
            .transport
            .execute(CrossPointAdapter::status_request())
            .await?;
        ensure_success(&response, "verify device connection")?;
        CrossPointAdapter::validate_status(&response.body)
    }
    async fn device_info(&self) -> Result<Vec<DeviceInfoField>, SdkError> {
        let response = self
            .transport
            .execute(CrossPointAdapter::status_request())
            .await?;
        ensure_success(&response, "get device info")?;
        CrossPointAdapter::parse_status(&response.body)
    }
    async fn list_files(&self, location: &FileLocation) -> Result<Vec<FileEntry>, SdkError> {
        self.list_crosspoint(location).await
    }
    async fn upload(&self, upload: FileUpload) -> Result<UploadResult, SdkError> {
        let path = join_location(&upload.location, &upload.file_name);
        let used_websocket = self
            .upload_crosspoint(
                &upload.local_path,
                &upload.file_name,
                &upload.location,
                &upload.options,
                upload.progress.clone(),
            )
            .await?;
        Ok(upload.result(path, used_websocket))
    }
    async fn delete(&self, path: &str) -> Result<(), SdkError> {
        self.execute_success(CrossPointAdapter::delete_request(path), "delete")
            .await
    }
    async fn delete_files(&self, paths: &[String]) -> Result<(), SdkError> {
        self.execute_success(
            CrossPointAdapter::delete_files_request(paths)?,
            "delete files",
        )
        .await
    }
    async fn download(&self, path: &str, destination: PathBuf) -> Result<(), SdkError> {
        self.transport
            .download(CrossPointAdapter::download_request(path), destination)
            .await
    }
    async fn create_directory(&self, parent: &str, name: &str) -> Result<(), SdkError> {
        self.execute_success(
            CrossPointAdapter::mkdir_request(parent, name),
            "create directory",
        )
        .await
    }
    async fn rename(&self, path: &str, new_name: &str) -> Result<(), SdkError> {
        self.execute_success(CrossPointAdapter::rename_request(path, new_name), "rename")
            .await
    }
    async fn move_file(&self, path: &str, destination: &str) -> Result<(), SdkError> {
        self.execute_success(CrossPointAdapter::move_request(path, destination), "move")
            .await
    }
    async fn list_wifi_networks(&self) -> Result<Vec<WifiNetwork>, SdkError> {
        let response = self
            .transport
            .execute(CrossPointAdapter::wifi_list_request())
            .await?;
        ensure_success(&response, "list Wi-Fi networks")?;
        CrossPointAdapter::parse_wifi_list(&response.text_lossy())
    }
    async fn save_wifi_network(&self, credential: &WifiCredential) -> Result<(), SdkError> {
        self.execute_success(
            CrossPointAdapter::wifi_save_request(credential)?,
            "save Wi-Fi network",
        )
        .await
    }
    async fn delete_wifi_network(&self, index: Option<u32>) -> Result<(), SdkError> {
        self.execute_success(
            CrossPointAdapter::wifi_delete_request(index.ok_or_else(|| {
                SdkError::InvalidArgument("CrossPoint Wi-Fi deletion requires an index".to_owned())
            })?)?,
            "delete Wi-Fi network",
        )
        .await
    }
    async fn list_fonts(&self) -> Result<FontCatalog, SdkError> {
        let response = self
            .transport
            .execute(CrossPointAdapter::fonts_list_request())
            .await?;
        ensure_success(&response, "list fonts")?;
        CrossPointAdapter::parse_font_catalog(&response.text_lossy())
    }
    async fn upload_font(&self, upload: FontUpload) -> Result<(), SdkError> {
        validate_font_family(&upload.family)?;
        if upload.overwrite {
            return Err(SdkError::Unsupported(
                "CrossPoint does not support explicit font overwrite".to_owned(),
            ));
        }
        let response = self
            .transport
            .execute_with_upload_progress(
                CrossPointAdapter::font_upload_request(
                    &upload.family,
                    upload.local_path,
                    upload.file_name,
                ),
                upload.progress,
            )
            .await?;
        ensure_success(&response, "upload font")
    }
    async fn delete_font_family(&self, family: &str) -> Result<(), SdkError> {
        validate_font_family(family)?;
        self.execute_success(
            CrossPointAdapter::font_delete_request(family)?,
            "delete font family",
        )
        .await
    }
    async fn list_opds_servers(&self) -> Result<Vec<OpdsServer>, SdkError> {
        let response = self
            .transport
            .execute(CrossPointAdapter::opds_list_request())
            .await?;
        ensure_success(&response, "list OPDS servers")?;
        CrossPointAdapter::parse_opds_list(&response.text_lossy())
    }
    async fn save_opds_server(&self, credential: &OpdsCredential) -> Result<(), SdkError> {
        self.execute_success(
            CrossPointAdapter::opds_save_request(credential)?,
            "save OPDS server",
        )
        .await
    }
    async fn delete_opds_server(&self, index: u32) -> Result<(), SdkError> {
        self.execute_success(
            CrossPointAdapter::opds_delete_request(index)?,
            "delete OPDS server",
        )
        .await
    }
    async fn list_settings(
        &self,
        policy: SettingsValidationPolicy,
    ) -> Result<SettingsSnapshot, SdkError> {
        let response = self
            .transport
            .execute(CrossPointAdapter::settings_list_request())
            .await?;
        ensure_success(&response, "list settings")?;
        parse_settings(&response.body, policy)
    }
    async fn apply_settings(
        &self,
        expected: SettingsSnapshot,
        changes: Vec<SettingChange>,
        policy: SettingsValidationPolicy,
    ) -> Result<SettingsSnapshot, SdkError> {
        let current = self.list_settings(policy).await?;
        if current != expected {
            return Err(SdkError::Conflict(
                "settings changed; reload before saving".to_owned(),
            ));
        }
        let body = encode_changes(&current, &changes)?;
        self.execute_success(
            CrossPointAdapter::settings_update_request(body),
            "update settings",
        )
        .await?;
        self.list_settings(policy).await.map_err(|error| {
            SdkError::CommittedWithWarning(format!("settings applied but refresh failed: {error}"))
        })
    }
}
