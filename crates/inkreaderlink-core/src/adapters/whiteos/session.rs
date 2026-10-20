//! Stateful `WhiteOsAdapter` protocol operations.
use crate::adapters::{DeviceAdapter, FileUpload};
use crate::model::ConflictPolicy;
use crate::transport::{HttpRequest, HttpResponse, SharedTransport};
use crate::validation::join_location;
use crate::{FileEntry, FileLocation, SdkError, UploadOptions, UploadResult};
use async_trait::async_trait;
use std::path::PathBuf;

use super::WhiteOsAdapter;
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
fn ensure_whiteos_success(response: &HttpResponse, operation: &str) -> Result<(), SdkError> {
    ensure_success(response, operation)?;
    let value = serde_json::from_slice::<serde_json::Value>(&response.body).ok();
    if value
        .as_ref()
        .and_then(|item| item.get("ok"))
        .and_then(serde_json::Value::as_bool)
        == Some(false)
    {
        let detail = value
            .as_ref()
            .and_then(|item| item.get("error"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or("device reported an unsuccessful operation");
        return Err(SdkError::RemoteFailure(format!(
            "{operation} failed: {detail}"
        )));
    }
    Ok(())
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
    async fn list_whiteos(&self, location: &FileLocation) -> Result<Vec<FileEntry>, SdkError> {
        let response = self
            .transport
            .execute(WhiteOsAdapter::list_request(location))
            .await?;
        ensure_success(&response, "list files")?;
        Ok(WhiteOsAdapter::parse_file_list(&response.text_lossy())?.entries)
    }
    async fn upload_whiteos(
        &self,
        local_path: PathBuf,
        file_name: &str,
        location: &FileLocation,
        options: UploadOptions,
    ) -> Result<(), SdkError> {
        match options.conflict_policy {
            ConflictPolicy::Fail => {
                if self
                    .list_whiteos(location)
                    .await?
                    .iter()
                    .any(|entry| entry.name == file_name)
                {
                    return Err(SdkError::Conflict(format!(
                        "remote file already exists: {file_name}"
                    )));
                }
            }
            ConflictPolicy::OverwriteWhenSupported => {
                return Err(SdkError::Unsupported(
                    "WhiteOS does not declare an overwrite operation".to_owned(),
                ));
            }
            ConflictPolicy::ReplaceWithBackup => {
                return Err(SdkError::Unsupported(
                    "WhiteOS does not declare backup replacement".to_owned(),
                ));
            }
        }

        let destination = join_location(location, file_name);
        let response = self
            .transport
            .execute(WhiteOsAdapter::upload_request(
                local_path,
                &destination,
                options.content_type,
            ))
            .await?;
        ensure_whiteos_success(&response, "upload")
    }
    async fn execute_success(&self, request: HttpRequest, operation: &str) -> Result<(), SdkError> {
        let response = self.transport.execute(request).await?;
        ensure_whiteos_success(&response, operation)
    }
}
#[async_trait]
impl DeviceAdapter for Session {
    async fn verify_connection(&self) -> Result<(), SdkError> {
        let response = self
            .transport
            .execute(WhiteOsAdapter::list_request(&FileLocation::Root))
            .await?;
        ensure_success(&response, "verify device connection")?;
        WhiteOsAdapter::validate_file_list(&response.body)
    }
    async fn list_files(&self, location: &FileLocation) -> Result<Vec<FileEntry>, SdkError> {
        self.list_whiteos(location).await
    }
    async fn upload(&self, upload: FileUpload) -> Result<UploadResult, SdkError> {
        let path = join_location(&upload.location, &upload.file_name);
        self.upload_whiteos(
            upload.local_path.clone(),
            &upload.file_name,
            &upload.location,
            upload.options.clone(),
        )
        .await?;
        Ok(upload.result(path, false))
    }
    async fn delete(&self, path: &str) -> Result<(), SdkError> {
        self.execute_success(WhiteOsAdapter::delete_request(path), "delete")
            .await
    }
    async fn download(&self, path: &str, destination: PathBuf) -> Result<(), SdkError> {
        self.transport
            .download(WhiteOsAdapter::download_request(path), destination)
            .await
    }
    async fn create_directory(&self, parent: &str, name: &str) -> Result<(), SdkError> {
        self.execute_success(
            WhiteOsAdapter::create_directory_request(&join_location(
                &FileLocation::Directory(parent.to_owned()),
                name,
            )),
            "create directory",
        )
        .await
    }
    async fn rename(&self, path: &str, new_name: &str) -> Result<(), SdkError> {
        let parent =
            path.rsplit_once('/').map_or(
                "/",
                |(parent, _)| if parent.is_empty() { "/" } else { parent },
            );
        self.execute_success(
            WhiteOsAdapter::rename_request(
                path,
                &join_location(&FileLocation::Directory(parent.to_owned()), new_name),
            ),
            "rename",
        )
        .await
    }
}
