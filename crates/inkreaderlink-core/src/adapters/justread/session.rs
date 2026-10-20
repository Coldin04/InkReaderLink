//! Stateful `JustReadAdapter` protocol operations.
use crate::adapters::{DeviceAdapter, FileUpload};
use crate::model::ConflictPolicy;
use crate::transport::{HttpResponse, SharedTransport};
use crate::{FileEntry, FileLocation, SdkError, UploadOptions, UploadProgressSink, UploadResult};
use async_trait::async_trait;
use std::{collections::HashMap, path::PathBuf, sync::Arc};

use super::JustReadAdapter;
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
fn ensure_justread_success(
    response: &HttpResponse,
    operation: &str,
    access_code: Option<&str>,
) -> Result<(), SdkError> {
    if response.status == 401 {
        return Err(if access_code.is_some() {
            SdkError::CredentialRejected {
                key: ACCESS_CODE_KEY.to_owned(),
            }
        } else {
            SdkError::CredentialRequired {
                key: ACCESS_CODE_KEY.to_owned(),
            }
        });
    }
    ensure_success(response, operation)
}
use super::ACCESS_CODE_KEY;
pub(crate) struct Session {
    transport: SharedTransport,
    access_code: Option<String>,
}
impl Session {
    pub fn new(transport: SharedTransport) -> Self {
        Self {
            transport,
            access_code: None,
        }
    }
}
impl Session {
    async fn list_justread(&self, location: &FileLocation) -> Result<Vec<FileEntry>, SdkError> {
        let request = JustReadAdapter::list_request(location, self.access_code.as_deref())?;
        let response = self.transport.execute(request).await?;
        ensure_justread_success(&response, "list files", self.access_code.as_deref())?;
        JustReadAdapter::parse_listing(location, &response.body)
    }
    async fn download_justread(&self, path: &str, destination: PathBuf) -> Result<(), SdkError> {
        let request = JustReadAdapter::download_ticket_request(path, self.access_code.as_deref())?;
        let response = self.transport.execute(request).await?;
        ensure_justread_success(&response, "request download", self.access_code.as_deref())?;
        let download = JustReadAdapter::download_request(&response.body)?;
        self.transport.download(download, destination).await
    }
    async fn upload_justread(
        &self,
        local_path: PathBuf,
        file_name: &str,
        location: &FileLocation,
        options: UploadOptions,
        progress: Option<Arc<dyn UploadProgressSink>>,
    ) -> Result<(), SdkError> {
        if options.conflict_policy != ConflictPolicy::Fail {
            return Err(SdkError::Unsupported(
                "JustRead does not support overwrite".to_owned(),
            ));
        }
        let destination = JustReadAdapter::upload_path(location, file_name)?;
        let request =
            JustReadAdapter::upload_request(local_path, &destination, self.access_code.as_deref())?;
        let response = self
            .transport
            .execute_with_upload_progress(request, progress)
            .await?;
        ensure_justread_success(&response, "upload", self.access_code.as_deref())?;
        let success: serde_json::Value =
            serde_json::from_slice(&response.body).map_err(|error| {
                SdkError::RemoteFailure(format!("invalid JustRead upload response: {error}"))
            })?;
        if success.get("ok").and_then(serde_json::Value::as_bool) != Some(true) {
            return Err(SdkError::RemoteFailure(
                "JustRead did not confirm the upload".to_owned(),
            ));
        }
        Ok(())
    }
}
#[async_trait]
impl DeviceAdapter for Session {
    async fn verify_connection(&self) -> Result<(), SdkError> {
        let response = self
            .transport
            .execute(JustReadAdapter::list_request(
                &FileLocation::Root,
                self.access_code.as_deref(),
            )?)
            .await?;
        ensure_justread_success(
            &response,
            "verify device connection",
            self.access_code.as_deref(),
        )?;
        JustReadAdapter::validate_listing(&response.body)
    }
    fn set_session_credentials(&mut self, credentials: HashMap<String, String>) {
        if let Some(code) = credentials.get(ACCESS_CODE_KEY) {
            self.access_code = Some(code.clone());
        }
    }
    async fn list_files(&self, location: &FileLocation) -> Result<Vec<FileEntry>, SdkError> {
        self.list_justread(location).await
    }
    async fn upload(&self, upload: FileUpload) -> Result<UploadResult, SdkError> {
        if upload.size > 0x7fff_0000 {
            return Err(SdkError::InvalidArgument(
                "JustRead upload exceeds the firmware size limit".to_owned(),
            ));
        }
        let path = JustReadAdapter::upload_path(&upload.location, &upload.file_name)?;
        self.upload_justread(
            upload.local_path.clone(),
            &upload.file_name,
            &upload.location,
            upload.options.clone(),
            upload.progress.clone(),
        )
        .await?;
        Ok(upload.result(path, false))
    }
    async fn download(&self, path: &str, destination: PathBuf) -> Result<(), SdkError> {
        self.download_justread(path, destination).await
    }
}
