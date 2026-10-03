use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use tokio::sync::Mutex;

use crate::{
    DeviceKind, DeviceProfile, FileDownload, FileEntry, FileKind, FileLocation, FontCatalog,
    OpdsCredential, OpdsServer, SdkError, SettingChange, SettingsSnapshot, UploadOptions,
    UploadProgressSink, UploadResult, WallpaperUploadResult, WifiCredential, WifiNetwork,
    adapters::{
        crosspoint::CrossPointAdapter, read_pico::ReadPicoAdapter,
        wegooo_cell_fork::WegoCellForkAdapter,
    },
    capability::ids,
    model::ConflictPolicy,
    settings::{encode_changes, parse_settings},
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

    /// Verifies the HTTP origin using the selected device's information endpoint.
    ///
    /// # Errors
    ///
    /// Returns transport, HTTP, or device response errors.
    pub async fn verify_connection(&self) -> Result<(), SdkError> {
        let request = match self.kind {
            DeviceKind::ReadPico => ReadPicoAdapter::info_request(),
            DeviceKind::CrossPoint => CrossPointAdapter::status_request(),
            DeviceKind::WegoCellFork => WegoCellForkAdapter::info_request(),
        };
        let response = self.transport.execute(request).await?;
        ensure_success(&response, "verify device connection")?;
        match self.kind {
            DeviceKind::ReadPico => ReadPicoAdapter::validate_info(&response.body),
            DeviceKind::CrossPoint => CrossPointAdapter::validate_status(&response.body),
            DeviceKind::WegoCellFork => WegoCellForkAdapter::validate_info(&response.body),
        }
    }

    /// Fetches the currently available information from the connected device.
    ///
    /// # Errors
    ///
    /// Returns `Unsupported` when the device has no information-page capability,
    /// or a transport, HTTP, or invalid-response error.
    pub async fn device_info(&self) -> Result<Vec<crate::DeviceInfoField>, SdkError> {
        self.require(ids::DEVICE_INFO)?;
        let request = match self.kind {
            DeviceKind::ReadPico => ReadPicoAdapter::info_request(),
            DeviceKind::CrossPoint => CrossPointAdapter::status_request(),
            DeviceKind::WegoCellFork => WegoCellForkAdapter::info_request(),
        };
        let response = self.transport.execute(request).await?;
        ensure_success(&response, "get device info")?;
        match self.kind {
            DeviceKind::ReadPico => ReadPicoAdapter::parse_info(&response.body),
            DeviceKind::CrossPoint => CrossPointAdapter::parse_status(&response.body),
            DeviceKind::WegoCellFork => WegoCellForkAdapter::parse_info(&response.body),
        }
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
            DeviceKind::WegoCellFork => self.list_wegooo_cell_fork(&location).await,
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
            DeviceKind::WegoCellFork => {
                self.upload_wegooo_cell_fork(local_path, file_name.clone(), options)
                    .await?;
                false
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
            DeviceKind::WegoCellFork => {
                let relative = path.strip_prefix('/').unwrap_or(&path);
                WegoCellForkAdapter::delete_path_request(relative)?
            }
        };
        self.execute_success(request, "delete").await
    }

    /// Deletes multiple remote files or empty directories.
    ///
    /// CrossPoint uses its multi-path endpoint. Read Pico executes its single-item
    /// endpoint serially and stops on the first failure.
    ///
    /// # Errors
    ///
    /// Returns validation, transport, protocol, or capability errors. A serial
    /// operation error reports how many paths completed before the failure.
    pub async fn delete_files(&self, paths: Vec<String>) -> Result<(), SdkError> {
        self.require(ids::FILE_DELETE)?;
        validate_batch_paths(&paths)?;

        if self.kind == DeviceKind::CrossPoint {
            let request = CrossPointAdapter::delete_files_request(&paths)?;
            let _guard = self.mutation.lock().await;
            return self.execute_success(request, "delete files").await;
        }

        let names = paths
            .iter()
            .map(|path| {
                if self.kind == DeviceKind::ReadPico {
                    read_pico_name(path).map(str::to_owned)
                } else {
                    Ok(path.strip_prefix('/').unwrap_or(path).to_owned())
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        let _guard = self.mutation.lock().await;
        for (index, (path, name)) in paths.iter().zip(names).enumerate() {
            let request = match self.kind {
                DeviceKind::ReadPico => ReadPicoAdapter::delete_request(&name),
                DeviceKind::WegoCellFork => WegoCellForkAdapter::delete_path_request(&name)?,
                DeviceKind::CrossPoint => unreachable!("CrossPoint exits through batch endpoint"),
            };
            if let Err(error) = self.execute_success(request, "delete").await {
                return Err(batch_operation_error(
                    "delete",
                    index,
                    paths.len(),
                    path,
                    error,
                ));
            }
        }
        Ok(())
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

    /// Downloads multiple remote files to their corresponding local destinations.
    /// Files are streamed one at a time and the operation stops on the first failure.
    ///
    /// # Errors
    ///
    /// Returns validation, file, transport, or capability errors. The error reports
    /// how many files completed before the failure; completed downloads are kept.
    pub async fn download_files(&self, files: Vec<FileDownload>) -> Result<(), SdkError> {
        self.require(ids::FILE_DOWNLOAD)?;
        if files.is_empty() {
            return Err(SdkError::InvalidArgument(
                "at least one file is required".to_owned(),
            ));
        }

        let mut remote_paths = HashSet::with_capacity(files.len());
        let mut destinations = HashSet::with_capacity(files.len());
        for file in &files {
            validate_remote_path(&file.path)?;
            if file.destination.as_os_str().is_empty() {
                return Err(SdkError::InvalidArgument(
                    "download destination must not be empty".to_owned(),
                ));
            }
            if !remote_paths.insert(file.path.as_str()) {
                return Err(SdkError::InvalidArgument(format!(
                    "duplicate remote path: {}",
                    file.path
                )));
            }
            if !destinations.insert(file.destination.as_path()) {
                return Err(SdkError::InvalidArgument(format!(
                    "duplicate download destination: {}",
                    file.destination.display()
                )));
            }
        }

        for (index, file) in files.iter().enumerate() {
            if let Err(error) = self
                .download(file.path.clone(), file.destination.clone())
                .await
            {
                return Err(batch_operation_error(
                    "download",
                    index,
                    files.len(),
                    &file.path,
                    error,
                ));
            }
        }
        Ok(())
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

    /// Moves multiple files into an existing directory, serially.
    ///
    /// # Errors
    ///
    /// Returns validation, transport, protocol, or capability errors. The first
    /// failure stops the operation and reports how many files already moved.
    pub async fn move_files(
        &self,
        paths: Vec<String>,
        destination: String,
    ) -> Result<(), SdkError> {
        self.require(ids::FILE_MOVE)?;
        validate_batch_paths(&paths)?;
        validate_remote_path(&destination)?;
        let _guard = self.mutation.lock().await;
        for (index, path) in paths.iter().enumerate() {
            let request = CrossPointAdapter::move_request(path, &destination);
            if let Err(error) = self.execute_success(request, "move").await {
                return Err(batch_operation_error(
                    "move",
                    index,
                    paths.len(),
                    path,
                    error,
                ));
            }
        }
        Ok(())
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
            DeviceKind::WegoCellFork => {
                self.transport
                    .execute(WegoCellForkAdapter::info_request())
                    .await?
            }
        };
        ensure_success(&response, "list Wi-Fi networks")?;
        let body = response.text_lossy();
        match self.kind {
            DeviceKind::ReadPico => ReadPicoAdapter::parse_wifi_info(&body),
            DeviceKind::CrossPoint => CrossPointAdapter::parse_wifi_list(&body),
            DeviceKind::WegoCellFork => WegoCellForkAdapter::parse_wifi_info(&body),
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
            DeviceKind::WegoCellFork => WegoCellForkAdapter::wifi_save_request(&credential)?,
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
            DeviceKind::WegoCellFork => WegoCellForkAdapter::wifi_delete_request(),
        };
        self.execute_success(request, "delete Wi-Fi network").await
    }

    /// Lists wallpaper images stored on the Fork firmware's TF card.
    ///
    /// # Errors
    ///
    /// Returns capability, transport, or protocol errors.
    pub async fn list_wallpapers(&self) -> Result<Vec<FileEntry>, SdkError> {
        self.require(ids::WALLPAPERS_MANAGE)?;
        let mut page_index = 0;
        let mut entries = Vec::new();
        loop {
            let response = self
                .transport
                .execute(WegoCellForkAdapter::wallpapers_list_request(page_index))
                .await?;
            ensure_wegooo_cell_fork_success(&response, "list wallpapers")?;
            let page = WegoCellForkAdapter::parse_wallpaper_page(&response.text_lossy())?;
            entries.extend(page.entries);
            page_index += 1;
            if page_index >= page.pages {
                return Ok(entries);
            }
        }
    }

    /// Uploads an image into the wallpaper library, optionally applying it as
    /// the lock-screen wallpaper in the same firmware request.
    ///
    /// # Errors
    ///
    /// Returns validation, capability, file, transport, conflict, or protocol errors.
    pub async fn upload_wallpaper(
        &self,
        local_path: PathBuf,
        file_name: String,
        overwrite: bool,
        apply_to_lock_screen: bool,
    ) -> Result<WallpaperUploadResult, SdkError> {
        self.require(ids::WALLPAPERS_UPLOAD)?;
        validate_file_name(&file_name)?;
        let extension = Path::new(&file_name)
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default();
        if !self
            .profile
            .file_formats
            .wallpaper_upload_extensions
            .iter()
            .any(|allowed| allowed.eq_ignore_ascii_case(extension))
        {
            return Err(SdkError::InvalidArgument(
                "wallpaper image extension is not supported by this device".to_owned(),
            ));
        }
        let size = tokio::fs::metadata(&local_path)
            .await
            .map_err(|error| {
                SdkError::InvalidArgument(format!("cannot read wallpaper image: {error}"))
            })?
            .len();
        if size == 0 {
            return Err(SdkError::InvalidArgument(
                "wallpaper image must not be empty".to_owned(),
            ));
        }
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

        let _guard = self.mutation.lock().await;
        let response = self
            .transport
            .execute(WegoCellForkAdapter::wallpaper_upload_request(
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
        ensure_wegooo_cell_fork_success(&response, "upload wallpaper")?;
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

    /// Deletes one image from the wallpaper library.
    ///
    /// # Errors
    ///
    /// Returns validation, capability, transport, or protocol errors.
    pub async fn delete_wallpaper(&self, file_name: String) -> Result<(), SdkError> {
        self.require(ids::WALLPAPERS_DELETE)?;
        validate_file_name(&file_name)?;
        if !self
            .profile
            .file_formats
            .wallpaper_upload_extensions
            .iter()
            .any(|allowed| {
                Path::new(&file_name)
                    .extension()
                    .and_then(|value| value.to_str())
                    .is_some_and(|extension| allowed.eq_ignore_ascii_case(extension))
            })
        {
            return Err(SdkError::InvalidArgument(
                "file is not a supported wallpaper image".to_owned(),
            ));
        }
        let _guard = self.mutation.lock().await;
        self.execute_success(
            WegoCellForkAdapter::delete_wallpaper_request(&file_name)?,
            "delete wallpaper",
        )
        .await
    }

    /// Lists installed SD-card font families.
    /// # Errors
    /// Returns capability, transport, or protocol errors.
    pub async fn list_fonts(&self) -> Result<FontCatalog, SdkError> {
        self.require(ids::FONTS_LIST)?;
        let response = self
            .transport
            .execute(CrossPointAdapter::fonts_list_request())
            .await?;
        ensure_success(&response, "list fonts")?;
        CrossPointAdapter::parse_font_catalog(&response.text_lossy())
    }

    /// Streams a device-supported font file. Read Pico rejects same-name files by default.
    /// # Errors
    /// Returns validation, capability, file, transport, or protocol errors.
    pub async fn upload_font(
        &self,
        family: String,
        local_path: PathBuf,
        file_name: String,
    ) -> Result<(), SdkError> {
        self.upload_font_with_overwrite(family, local_path, file_name, false)
            .await
    }

    /// Streams a font file, replacing a same-name Read Pico font only when requested.
    /// # Errors
    /// Returns validation, capability, file, transport, or protocol errors.
    pub async fn upload_font_with_overwrite(
        &self,
        family: String,
        local_path: PathBuf,
        file_name: String,
        overwrite: bool,
    ) -> Result<(), SdkError> {
        self.upload_font_with_overwrite_and_progress(family, local_path, file_name, overwrite, None)
            .await
    }

    /// Streams a font file and reports bytes supplied to the HTTP request body.
    ///
    /// # Errors
    ///
    /// Returns validation, capability, file, transport, or protocol errors.
    pub async fn upload_font_with_progress(
        &self,
        family: String,
        local_path: PathBuf,
        file_name: String,
        progress: Arc<dyn UploadProgressSink>,
    ) -> Result<(), SdkError> {
        self.upload_font_with_overwrite_and_progress(
            family,
            local_path,
            file_name,
            false,
            Some(progress),
        )
        .await
    }

    pub async fn upload_font_with_overwrite_and_progress(
        &self,
        family: String,
        local_path: PathBuf,
        file_name: String,
        overwrite: bool,
        progress: Option<Arc<dyn UploadProgressSink>>,
    ) -> Result<(), SdkError> {
        self.require(ids::FONTS_UPLOAD)?;
        if progress.is_some() {
            self.require(ids::FONTS_UPLOAD_PROGRESS)?;
        }
        validate_file_name(&file_name)?;
        let extension = Path::new(&file_name)
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default();
        if !self
            .profile
            .file_formats
            .font_upload_extensions
            .iter()
            .any(|allowed| allowed.eq_ignore_ascii_case(extension))
        {
            return Err(SdkError::InvalidArgument(
                "font file extension is not supported by this device".to_owned(),
            ));
        }
        if self.kind == DeviceKind::CrossPoint {
            validate_font_family(&family)?;
            if overwrite {
                return Err(SdkError::Unsupported(
                    "CrossPoint does not support explicit font overwrite".to_owned(),
                ));
            }
        } else if file_name.len() > 120 {
            return Err(SdkError::InvalidArgument(
                "Read Pico font file name exceeds 120 bytes".to_owned(),
            ));
        }
        let size = tokio::fs::metadata(&local_path)
            .await
            .map_err(|error| SdkError::InvalidArgument(format!("cannot read font file: {error}")))?
            .len();
        if size == 0 {
            return Err(SdkError::InvalidArgument(
                "font file must not be empty".to_owned(),
            ));
        }
        if self.kind != DeviceKind::CrossPoint && size > 32 * 1024 * 1024 {
            return Err(SdkError::InvalidArgument(
                "font file exceeds 32 MiB".to_owned(),
            ));
        }
        let _guard = self.mutation.lock().await;
        if self.kind == DeviceKind::ReadPico && !overwrite {
            // The firmware closes the connection after rejecting a PUT whose
            // body is still unread. Probe the documented GET endpoint first
            // so an existing filename becomes a reliable conflict response.
            let existing = self
                .transport
                .execute(ReadPicoAdapter::font_info_request(file_name.clone()))
                .await?;
            match existing.status {
                200 => {
                    return Err(SdkError::Conflict(format!(
                        "font already exists: {file_name}"
                    )));
                }
                404 => {}
                409 => {
                    return Err(SdkError::Unsupported(
                        "Read Pico font upload requires a mounted TF card".to_owned(),
                    ));
                }
                _ => ensure_read_pico_success(&existing, "check font before upload")?,
            }
        }
        let request = match self.kind {
            DeviceKind::ReadPico => {
                ReadPicoAdapter::font_upload_request(local_path, file_name, overwrite)
            }
            DeviceKind::CrossPoint => {
                CrossPointAdapter::font_upload_request(&family, local_path, file_name)
            }
            DeviceKind::WegoCellFork => {
                WegoCellForkAdapter::font_upload_request(local_path, file_name, overwrite)
            }
        };
        let response = self
            .transport
            .execute_with_upload_progress(request, progress)
            .await?;
        if self.kind == DeviceKind::ReadPico && response.status == 409 {
            let error = serde_json::from_slice::<serde_json::Value>(&response.body).ok();
            if error
                .as_ref()
                .and_then(|value| value.get("conflict"))
                .and_then(serde_json::Value::as_bool)
                != Some(true)
            {
                return Err(SdkError::Unsupported(
                    "Read Pico font upload requires a mounted TF card".to_owned(),
                ));
            }
        }
        match self.kind {
            DeviceKind::ReadPico => ensure_read_pico_success(&response, "upload font"),
            DeviceKind::CrossPoint => ensure_success(&response, "upload font"),
            DeviceKind::WegoCellFork => ensure_wegooo_cell_fork_success(&response, "upload font"),
        }
    }

    /// Deletes an installed font family.
    /// # Errors
    /// Returns validation, capability, transport, or protocol errors.
    pub async fn delete_font_family(&self, family: String) -> Result<(), SdkError> {
        self.require(ids::FONTS_DELETE)?;
        validate_font_family(&family)?;
        let _guard = self.mutation.lock().await;
        self.execute_success(
            CrossPointAdapter::font_delete_request(&family)?,
            "delete font family",
        )
        .await
    }

    /// Lists saved OPDS servers without passwords.
    /// # Errors
    /// Returns capability, transport, or protocol errors.
    pub async fn list_opds_servers(&self) -> Result<Vec<OpdsServer>, SdkError> {
        self.require(ids::OPDS_LIST)?;
        let response = self
            .transport
            .execute(CrossPointAdapter::opds_list_request())
            .await?;
        ensure_success(&response, "list OPDS servers")?;
        CrossPointAdapter::parse_opds_list(&response.text_lossy())
    }

    /// Adds or updates an OPDS server. Omitted password preserves it on update.
    /// # Errors
    /// Returns validation, capability, transport, or protocol errors.
    pub async fn save_opds_server(&self, credential: OpdsCredential) -> Result<(), SdkError> {
        self.require(ids::OPDS_SAVE)?;
        if credential.name.trim().is_empty() || credential.url.trim().is_empty() {
            return Err(SdkError::InvalidArgument(
                "OPDS name and URL must not be empty".to_owned(),
            ));
        }
        let _guard = self.mutation.lock().await;
        self.execute_success(
            CrossPointAdapter::opds_save_request(&credential)?,
            "save OPDS server",
        )
        .await
    }

    /// Deletes a saved OPDS server by index.
    /// # Errors
    /// Returns capability, transport, or protocol errors.
    pub async fn delete_opds_server(&self, index: u32) -> Result<(), SdkError> {
        self.require(ids::OPDS_DELETE)?;
        let _guard = self.mutation.lock().await;
        self.execute_success(
            CrossPointAdapter::opds_delete_request(index)?,
            "delete OPDS server",
        )
        .await
    }

    /// Returns the current editable settings and metadata needed to render their controls.
    ///
    /// # Errors
    /// Returns capability, transport, or malformed response errors.
    pub async fn list_settings(&self) -> Result<SettingsSnapshot, SdkError> {
        self.require(ids::SETTINGS_LIST)?;
        self.read_settings().await
    }

    /// Applies a partial update only if the setting descriptors have not changed.
    ///
    /// # Errors
    /// Returns capability, conflict, validation, transport, or protocol errors.
    pub async fn apply_settings(
        &self,
        expected: SettingsSnapshot,
        changes: Vec<SettingChange>,
    ) -> Result<SettingsSnapshot, SdkError> {
        self.require(ids::SETTINGS_UPDATE)?;
        self.require(ids::SETTINGS_LIST)?;
        let _guard = self.mutation.lock().await;
        let current = self.read_settings().await?;
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
        self.read_settings().await.map_err(|error| {
            SdkError::CommittedWithWarning(format!("settings applied but refresh failed: {error}"))
        })
    }

    async fn read_settings(&self) -> Result<SettingsSnapshot, SdkError> {
        let response = self
            .transport
            .execute(CrossPointAdapter::settings_list_request())
            .await?;
        ensure_success(&response, "list settings")?;
        parse_settings(&response.body)
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

    async fn list_wegooo_cell_fork(
        &self,
        location: &FileLocation,
    ) -> Result<Vec<FileEntry>, SdkError> {
        let mut page_index = 0;
        let mut entries = Vec::new();
        loop {
            let response = self
                .transport
                .execute(WegoCellForkAdapter::file_list_request(location, page_index))
                .await?;
            ensure_wegooo_cell_fork_success(&response, "list files")?;
            let page = WegoCellForkAdapter::parse_file_page(location, &response.text_lossy())?;
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

    async fn upload_wegooo_cell_fork(
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
                    "wegooo-cell-fork uses the firmware's explicit overwrite operation".to_owned(),
                ));
            }
        };
        let response = self
            .transport
            .execute(WegoCellForkAdapter::book_upload_request(
                local_path, file_name, overwrite,
            ))
            .await?;
        ensure_wegooo_cell_fork_success(&response, "upload")
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
            DeviceKind::WegoCellFork => ensure_wegooo_cell_fork_success(&response, operation),
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

fn ensure_wegooo_cell_fork_success(
    response: &HttpResponse,
    operation: &str,
) -> Result<(), SdkError> {
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

fn validate_font_family(family: &str) -> Result<(), SdkError> {
    validate_file_name(family)?;
    if family.trim() != family || family.starts_with('.') {
        return Err(SdkError::InvalidArgument(
            "invalid font family name".to_owned(),
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

fn validate_batch_paths(paths: &[String]) -> Result<(), SdkError> {
    if paths.is_empty() {
        return Err(SdkError::InvalidArgument(
            "at least one file is required".to_owned(),
        ));
    }
    let mut unique = HashSet::with_capacity(paths.len());
    for path in paths {
        validate_remote_path(path)?;
        if !unique.insert(path.as_str()) {
            return Err(SdkError::InvalidArgument(format!(
                "duplicate remote path: {path}"
            )));
        }
    }
    Ok(())
}

fn batch_operation_error(
    operation: &str,
    completed: usize,
    total: usize,
    path: &str,
    error: SdkError,
) -> SdkError {
    SdkError::RemoteFailure(format!(
        "batch {operation} stopped after {completed}/{total} files; failed at {path}: {error}. Earlier completed files were not rolled back."
    ))
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
    use crate::transport::{HttpMethod, HttpRequest, Transport};

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
    async fn read_pico_font_upload_uses_font_endpoint_and_preserves_conflict() {
        let transport = MockTransport::new(vec![
            response(200, r#"{"name":"test.ttf","size":9}"#),
            response(200, r#"{"ok":true}"#),
        ]);
        let client = DeviceClient::with_transport(DeviceKind::ReadPico, transport.clone());
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(b"font data").unwrap();
        let path = file.path().to_path_buf();
        let first = client
            .upload_font(String::new(), path.clone(), "test.ttf".to_owned())
            .await;
        assert!(matches!(first, Err(SdkError::Conflict(_))));
        client
            .upload_font_with_overwrite(String::new(), path, "test.ttf".to_owned(), true)
            .await
            .unwrap();
        let requests = transport.requests.lock().unwrap();
        assert_eq!(requests[0].method, HttpMethod::Get);
        assert_eq!(requests[0].path, "/fonts");
        assert!(
            requests[0]
                .query
                .iter()
                .any(|(key, value)| key == "name" && value == "test.ttf")
        );
        assert_eq!(requests[1].method, HttpMethod::Put);
        assert_eq!(requests[1].path, "/fonts");
        assert!(
            requests[1]
                .query
                .iter()
                .any(|(key, value)| key == "overwrite" && value == "1")
        );
    }

    #[tokio::test]
    async fn read_pico_font_upload_reports_missing_card() {
        let transport = MockTransport::new(vec![response(409, r#"{"error":"字体需要 TF 卡"}"#)]);
        let client = DeviceClient::with_transport(DeviceKind::ReadPico, transport);
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(b"font data").unwrap();
        let result = client
            .upload_font(
                String::new(),
                file.path().to_path_buf(),
                "test.ttf".to_owned(),
            )
            .await;
        assert!(matches!(result, Err(SdkError::Unsupported(_))));
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

    #[tokio::test]
    async fn settings_reject_stale_snapshot_before_post() {
        let old = r#"[{"key":"fontSize","name":"Font size","category":"Reader","type":"enum","value":0,"options":["12 pt","14 pt"]}]"#;
        let changed = r#"[{"key":"fontSize","name":"Font size","category":"Reader","type":"enum","value":0,"options":["10 pt","12 pt"]}]"#;
        let transport = MockTransport::new(vec![response(200, old), response(200, changed)]);
        let client = DeviceClient::with_transport(DeviceKind::CrossPoint, transport.clone());
        let snapshot = client.list_settings().await.unwrap();
        let result = client
            .apply_settings(
                snapshot,
                vec![SettingChange {
                    key: "fontSize".to_owned(),
                    value: crate::SettingValue::Choice(1),
                }],
            )
            .await;
        assert!(matches!(result, Err(SdkError::Conflict(_))));
        assert_eq!(transport.request_count(), 2);
    }

    #[tokio::test]
    async fn read_pico_settings_are_capability_gated() {
        let transport = MockTransport::new(Vec::new());
        let client = DeviceClient::with_transport(DeviceKind::ReadPico, transport.clone());
        assert!(matches!(
            client.list_settings().await,
            Err(SdkError::Unsupported(_))
        ));
        assert_eq!(transport.request_count(), 0);
    }

    #[tokio::test]
    async fn settings_post_only_changed_values_and_return_refreshed_snapshot() {
        let old = r#"[{"key":"showHiddenFiles","name":"Show hidden files","category":"Files","type":"toggle","value":0}]"#;
        let new = r#"[{"key":"showHiddenFiles","name":"Show hidden files","category":"Files","type":"toggle","value":1}]"#;
        let transport = MockTransport::new(vec![
            response(200, old),
            response(200, old),
            response(200, "Applied 1 setting(s)"),
            response(200, new),
        ]);
        let client = DeviceClient::with_transport(DeviceKind::CrossPoint, transport.clone());
        let snapshot = client.list_settings().await.unwrap();
        let updated = client
            .apply_settings(
                snapshot,
                vec![SettingChange {
                    key: "showHiddenFiles".to_owned(),
                    value: crate::SettingValue::Toggle(true),
                }],
            )
            .await
            .unwrap();
        assert_eq!(updated.settings[0].value, crate::SettingValue::Toggle(true));
        let requests = transport.requests.lock().unwrap();
        assert_eq!(requests.len(), 4);
        assert_eq!(requests[2].path, "/api/settings");
        assert!(
            matches!(&requests[2].body, crate::transport::HttpBody::Json(body)
            if serde_json::from_slice::<serde_json::Value>(body).unwrap() == serde_json::json!({"showHiddenFiles":1}))
        );
    }
}
