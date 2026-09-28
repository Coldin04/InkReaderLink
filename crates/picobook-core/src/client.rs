use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use tokio::sync::Mutex;

use crate::{
    DeviceKind, DeviceProfile, FileEntry, FileKind, FileLocation, SdkError, UploadOptions,
    UploadProgressSink, UploadResult, WifiCredential, WifiNetwork,
    adapters::{crosspoint::CrossPointAdapter, read_pico::ReadPicoAdapter},
    capability::ids,
    model::ConflictPolicy,
    transport::{HttpResponse, ReqwestTransport, SharedTransport, WebSocketUpload},
};

#[derive(Debug)]
pub struct DeviceClient {
    kind: DeviceKind,
    profile: DeviceProfile,
    transport: SharedTransport,
    mutation: Mutex<()>,
}

impl DeviceClient {
    /// Creates a client for a known device and HTTP origin.
    ///
    /// # Errors
    ///
    /// Returns an error when the device URL is invalid.
    pub fn connect(kind: DeviceKind, base_url: &str, timeout: Duration) -> Result<Self, SdkError> {
        let transport = Arc::new(ReqwestTransport::new(base_url, timeout)?);
        Ok(Self::with_transport(kind, transport))
    }

    #[must_use]
    pub fn with_transport(kind: DeviceKind, transport: SharedTransport) -> Self {
        Self {
            kind,
            profile: DeviceProfile::baseline(kind),
            transport,
            mutation: Mutex::new(()),
        }
    }

    #[must_use]
    pub fn profile(&self) -> &DeviceProfile {
        &self.profile
    }

    /// Lists files at a supported location.
    ///
    /// # Errors
    ///
    /// Returns protocol, transport, or capability errors.
    pub async fn list_files(&self, location: FileLocation) -> Result<Vec<FileEntry>, SdkError> {
        self.require(ids::FILE_LIST)?;
        if matches!(location, FileLocation::Directory(_)) {
            self.require(ids::FILE_DIRECTORY_LIST)?;
        }
        match self.kind {
            DeviceKind::ReadPico => self.list_read_pico(location).await,
            DeviceKind::CrossPoint => self.list_crosspoint(&location).await,
        }
    }

    /// Uploads a local file without buffering it in memory.
    ///
    /// # Errors
    ///
    /// Returns protocol, file, transport, conflict, or capability errors.
    pub async fn upload(
        &self,
        local_path: PathBuf,
        file_name: String,
        location: FileLocation,
        options: UploadOptions,
        progress: Option<Arc<dyn UploadProgressSink>>,
    ) -> Result<UploadResult, SdkError> {
        self.require(ids::FILE_UPLOAD)?;
        validate_file_name(&file_name)?;
        self.validate_upload_format(&file_name)?;
        self.validate_location(&location)?;
        let size = tokio::fs::metadata(&local_path)
            .await
            .map_err(|error| {
                SdkError::InvalidArgument(format!("cannot read upload file: {error}"))
            })?
            .len();
        if size == 0 {
            return Err(SdkError::InvalidArgument(
                "upload file must not be empty".to_owned(),
            ));
        }

        let _guard = self.mutation.lock().await;
        let used_websocket = match self.kind {
            DeviceKind::ReadPico => {
                self.upload_read_pico(local_path, file_name.clone(), options)
                    .await?;
                false
            }
            DeviceKind::CrossPoint => {
                self.upload_crosspoint(&local_path, &file_name, &location, &options, progress)
                    .await?
            }
        };
        let path = join_location(&location, &file_name);
        Ok(UploadResult {
            entry: FileEntry {
                name: file_name.clone(),
                path,
                size,
                kind: infer_file_kind(&file_name),
            },
            used_websocket,
        })
    }

    /// Deletes a remote file or empty directory.
    ///
    /// # Errors
    ///
    /// Returns validation, transport, protocol, or capability errors.
    pub async fn delete(&self, path: String) -> Result<(), SdkError> {
        self.require(ids::FILE_DELETE)?;
        validate_remote_path(&path)?;
        let _guard = self.mutation.lock().await;
        let request = match self.kind {
            DeviceKind::ReadPico => ReadPicoAdapter::delete_request(read_pico_name(&path)?),
            DeviceKind::CrossPoint => CrossPointAdapter::delete_request(&path),
        };
        self.execute_success(request, "delete").await
    }

    /// Downloads a remote `CrossPoint` file to a local path using a temporary file.
    ///
    /// # Errors
    ///
    /// Returns file, transport, protocol, or capability errors.
    pub async fn download(&self, path: String, destination: PathBuf) -> Result<(), SdkError> {
        self.require(ids::FILE_DOWNLOAD)?;
        validate_remote_path(&path)?;
        self.transport
            .download(CrossPointAdapter::download_request(&path), destination)
            .await
    }

    /// Creates a remote directory.
    ///
    /// # Errors
    ///
    /// Returns validation, transport, protocol, or capability errors.
    pub async fn create_directory(&self, parent: String, name: String) -> Result<(), SdkError> {
        self.require(ids::DIRECTORY_CREATE)?;
        validate_remote_path(&parent)?;
        validate_file_name(&name)?;
        let _guard = self.mutation.lock().await;
        self.execute_success(
            CrossPointAdapter::mkdir_request(&parent, &name),
            "create directory",
        )
        .await
    }

    /// Renames a remote file.
    ///
    /// # Errors
    ///
    /// Returns validation, transport, protocol, or capability errors.
    pub async fn rename(&self, path: String, new_name: String) -> Result<(), SdkError> {
        self.require(ids::FILE_RENAME)?;
        validate_remote_path(&path)?;
        validate_file_name(&new_name)?;
        let _guard = self.mutation.lock().await;
        self.execute_success(
            CrossPointAdapter::rename_request(&path, &new_name),
            "rename",
        )
        .await
    }

    /// Moves a remote file to an existing directory.
    ///
    /// # Errors
    ///
    /// Returns validation, transport, protocol, or capability errors.
    pub async fn move_file(&self, path: String, destination: String) -> Result<(), SdkError> {
        self.require(ids::FILE_MOVE)?;
        validate_remote_path(&path)?;
        validate_remote_path(&destination)?;
        let _guard = self.mutation.lock().await;
        self.execute_success(CrossPointAdapter::move_request(&path, &destination), "move")
            .await
    }

    /// Lists saved Wi-Fi networks visible through the firmware API.
    ///
    /// # Errors
    ///
    /// Returns transport, protocol, or capability errors.
    pub async fn list_wifi_networks(&self) -> Result<Vec<WifiNetwork>, SdkError> {
        self.require(ids::WIFI_LIST)?;
        let response = match self.kind {
            DeviceKind::ReadPico => {
                self.transport
                    .execute(ReadPicoAdapter::info_request())
                    .await?
            }
            DeviceKind::CrossPoint => {
                self.transport
                    .execute(CrossPointAdapter::wifi_list_request())
                    .await?
            }
        };
        ensure_success(&response, "list Wi-Fi networks")?;
        let body = response.text_lossy();
        match self.kind {
            DeviceKind::ReadPico => ReadPicoAdapter::parse_wifi_info(&body),
            DeviceKind::CrossPoint => CrossPointAdapter::parse_wifi_list(&body),
        }
    }

    /// Adds or updates a saved Wi-Fi network.
    ///
    /// # Errors
    ///
    /// Returns validation, transport, protocol, or capability errors.
    pub async fn save_wifi_network(&self, credential: WifiCredential) -> Result<(), SdkError> {
        self.require(ids::WIFI_SAVE)?;
        if credential.ssid.is_empty() {
            return Err(SdkError::InvalidArgument(
                "SSID must not be empty".to_owned(),
            ));
        }
        let _guard = self.mutation.lock().await;
        let request = match self.kind {
            DeviceKind::ReadPico => ReadPicoAdapter::wifi_save_request(&credential)?,
            DeviceKind::CrossPoint => CrossPointAdapter::wifi_save_request(&credential)?,
        };
        self.execute_success(request, "save Wi-Fi network").await
    }

    /// Deletes a saved Wi-Fi network.
    ///
    /// # Errors
    ///
    /// Returns validation, transport, protocol, or capability errors.
    pub async fn delete_wifi_network(&self, index: Option<u32>) -> Result<(), SdkError> {
        self.require(ids::WIFI_DELETE)?;
        let _guard = self.mutation.lock().await;
        let request = match self.kind {
            DeviceKind::ReadPico => ReadPicoAdapter::wifi_delete_request(),
            DeviceKind::CrossPoint => {
                CrossPointAdapter::wifi_delete_request(index.ok_or_else(|| {
                    SdkError::InvalidArgument(
                        "CrossPoint Wi-Fi deletion requires an index".to_owned(),
                    )
                })?)?
            }
        };
        self.execute_success(request, "delete Wi-Fi network").await
    }

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

    async fn list_crosspoint(&self, location: &FileLocation) -> Result<Vec<FileEntry>, SdkError> {
        let response = self
            .transport
            .execute(CrossPointAdapter::list_request(location))
            .await?;
        ensure_success(&response, "list files")?;
        Ok(CrossPointAdapter::parse_file_list(location, &response.text_lossy())?.entries)
    }

    async fn upload_read_pico(
        &self,
        local_path: PathBuf,
        file_name: String,
        options: UploadOptions,
    ) -> Result<(), SdkError> {
        let overwrite = match options.conflict_policy {
            ConflictPolicy::Fail => false,
            ConflictPolicy::OverwriteWhenSupported => {
                self.require(ids::UPLOAD_EXPLICIT_OVERWRITE)?;
                true
            }
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
                self.require(ids::UPLOAD_BACKUP_REPLACE)?;
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
            self.require(ids::UPLOAD_WEBSOCKET)?;
            self.transport
                .upload_websocket(WebSocketUpload {
                    file_path: local_path.to_path_buf(),
                    file_name: file_name.to_owned(),
                    destination: location_string(location),
                    progress,
                })
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

    async fn execute_success(
        &self,
        request: crate::transport::HttpRequest,
        operation: &str,
    ) -> Result<(), SdkError> {
        let response = self.transport.execute(request).await?;
        match self.kind {
            DeviceKind::ReadPico => ensure_read_pico_success(&response, operation),
            DeviceKind::CrossPoint => ensure_success(&response, operation),
        }
    }

    fn require(&self, capability: &str) -> Result<(), SdkError> {
        if self
            .profile
            .capabilities
            .iter()
            .any(|item| item.id() == capability)
        {
            Ok(())
        } else {
            Err(SdkError::Unsupported(format!(
                "device does not declare {capability}"
            )))
        }
    }

    fn validate_location(&self, location: &FileLocation) -> Result<(), SdkError> {
        if matches!(location, FileLocation::Directory(_)) {
            self.require(ids::UPLOAD_DIRECTORY_TARGET)?;
        }
        Ok(())
    }

    fn validate_upload_format(&self, file_name: &str) -> Result<(), SdkError> {
        if self.profile.file_formats.accepts_any_upload_format {
            return Ok(());
        }
        let extension = Path::new(file_name)
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        if self
            .profile
            .file_formats
            .upload_extensions
            .iter()
            .any(|allowed| allowed.eq_ignore_ascii_case(&extension))
        {
            Ok(())
        } else {
            Err(SdkError::Unsupported(format!(
                "device does not accept .{extension} uploads"
            )))
        }
    }
}

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

fn validate_file_name(name: &str) -> Result<(), SdkError> {
    if name.is_empty() || name == "." || name == ".." || name.contains(['/', '\\']) {
        return Err(SdkError::InvalidArgument(
            "file name must be a single non-empty path component".to_owned(),
        ));
    }
    Ok(())
}

fn validate_remote_path(path: &str) -> Result<(), SdkError> {
    if !path.starts_with('/') || path.contains("/../") || path.ends_with("/..") {
        return Err(SdkError::InvalidArgument(
            "remote path must be absolute and must not traverse parents".to_owned(),
        ));
    }
    Ok(())
}

fn read_pico_name(path: &str) -> Result<&str, SdkError> {
    let name = path.strip_prefix('/').unwrap_or(path);
    validate_file_name(name)?;
    Ok(name)
}

fn location_string(location: &FileLocation) -> String {
    match location {
        FileLocation::Root => "/".to_owned(),
        FileLocation::Directory(path) => path.clone(),
    }
}

fn join_location(location: &FileLocation, name: &str) -> String {
    let base = location_string(location);
    if base == "/" {
        format!("/{name}")
    } else {
        format!("{}/{name}", base.trim_end_matches('/'))
    }
}

fn infer_file_kind(name: &str) -> FileKind {
    match Path::new(name)
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("epub" | "txt" | "md" | "xtc") => FileKind::Book,
        _ => FileKind::Other,
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::VecDeque,
        io::Write,
        sync::{Arc, Mutex as StdMutex},
    };

    use async_trait::async_trait;
    use tempfile::NamedTempFile;

    use super::*;
    use crate::transport::{HttpRequest, Transport};

    #[derive(Debug)]
    struct MockTransport {
        responses: StdMutex<VecDeque<HttpResponse>>,
        requests: StdMutex<Vec<HttpRequest>>,
    }

    impl MockTransport {
        fn new(responses: Vec<HttpResponse>) -> Arc<Self> {
            Arc::new(Self {
                responses: StdMutex::new(responses.into()),
                requests: StdMutex::new(Vec::new()),
            })
        }

        fn request_count(&self) -> usize {
            self.requests.lock().unwrap().len()
        }
    }

    #[async_trait]
    impl Transport for MockTransport {
        async fn execute(&self, request: HttpRequest) -> Result<HttpResponse, SdkError> {
            self.requests.lock().unwrap().push(request);
            self.responses
                .lock()
                .unwrap()
                .pop_front()
                .ok_or_else(|| SdkError::RemoteFailure("missing mock response".to_owned()))
        }

        async fn download(
            &self,
            _request: HttpRequest,
            _destination: PathBuf,
        ) -> Result<(), SdkError> {
            Err(SdkError::RemoteFailure(
                "unexpected mock download".to_owned(),
            ))
        }
    }

    fn response(status: u16, body: &str) -> HttpResponse {
        HttpResponse {
            status,
            body: body.as_bytes().to_vec(),
        }
    }

    #[tokio::test]
    async fn read_pico_file_pages_are_hidden_from_callers() {
        let transport = MockTransport::new(vec![
            response(
                200,
                r#"{"page":0,"total":2,"pages":2,"items":[{"name":"a.epub","size":1}]}"#,
            ),
            response(
                200,
                r#"{"page":1,"total":2,"pages":2,"items":[{"name":"b.txt","size":2}]}"#,
            ),
        ]);
        let client = DeviceClient::with_transport(DeviceKind::ReadPico, transport.clone());

        let files = client.list_files(FileLocation::Root).await.unwrap();

        assert_eq!(files.len(), 2);
        assert_eq!(transport.request_count(), 2);
    }

    #[tokio::test]
    async fn unsupported_capability_does_not_send_a_request() {
        let transport = MockTransport::new(Vec::new());
        let client = DeviceClient::with_transport(DeviceKind::ReadPico, transport.clone());

        let result = client
            .download("/book.epub".to_owned(), PathBuf::from("unused"))
            .await;

        assert!(matches!(result, Err(SdkError::Unsupported(_))));
        assert_eq!(transport.request_count(), 0);
    }

    #[tokio::test]
    async fn crosspoint_backup_replace_verifies_before_cleanup() {
        let transport = MockTransport::new(vec![
            response(
                200,
                r#"[{"name":"book.epub","size":3,"isDirectory":false,"isEpub":true}]"#,
            ),
            response(200, "renamed"),
            response(200, "uploaded"),
            response(
                200,
                r#"[{"name":"book.epub","size":4,"isDirectory":false,"isEpub":true},{"name":"book.epub.back","size":3,"isDirectory":false,"isEpub":false}]"#,
            ),
            response(200, "deleted"),
        ]);
        let client = DeviceClient::with_transport(DeviceKind::CrossPoint, transport.clone());
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(b"book").unwrap();

        let result = client
            .upload(
                file.path().to_path_buf(),
                "book.epub".to_owned(),
                FileLocation::Directory("/Books".to_owned()),
                UploadOptions {
                    conflict_policy: ConflictPolicy::ReplaceWithBackup,
                    content_type: None,
                    prefer_websocket: false,
                },
                None,
            )
            .await
            .unwrap();

        assert_eq!(result.entry.path, "/Books/book.epub");
        assert_eq!(transport.request_count(), 5);
    }
}
