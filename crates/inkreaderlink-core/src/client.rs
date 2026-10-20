use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use tokio::sync::Mutex;

use crate::{
    DeviceDefinition, DeviceKind, DeviceProfile, FileDownload, FileEntry, FileLocation,
    FontCatalog, OpdsCredential, OpdsServer, SdkError, SettingChange, SettingsSnapshot,
    UploadOptions, UploadProgressSink, UploadResult, WallpaperUploadResult, WifiCredential,
    WifiNetwork,
    adapters::{self, DeviceAdapter, FileUpload, FontUpload, WallpaperUpload},
    capability::ids,
    model::{ConflictPolicy, SettingsValidationPolicy},
    transport::{ReqwestTransport, SharedTransport},
    validation::{
        batch_operation_error, validate_batch_paths, validate_file_name, validate_remote_path,
    },
};

pub struct DeviceClient {
    profile: DeviceProfile,
    adapter: Box<dyn DeviceAdapter>,
    mutation: Mutex<()>,
    settings_validation_policy: SettingsValidationPolicy,
}

impl std::fmt::Debug for DeviceClient {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DeviceClient")
            .field("profile", &self.profile)
            .finish_non_exhaustive()
    }
}

impl DeviceClient {
    /// Creates a client for a known device and HTTP origin.
    ///
    /// # Errors
    ///
    /// Returns an error when the device URL is invalid.
    pub fn connect(kind: DeviceKind, base_url: &str, timeout: Duration) -> Result<Self, SdkError> {
        let transport = Arc::new(ReqwestTransport::new(base_url, timeout)?);
        let mut adapter = adapters::create(kind, transport);
        adapter.configure_address(base_url)?;
        Ok(Self {
            profile: DeviceProfile::baseline(kind),
            adapter,
            mutation: Mutex::new(()),
            settings_validation_policy: SettingsValidationPolicy::Permissive,
        })
    }

    #[must_use]
    pub fn with_transport(kind: DeviceKind, transport: SharedTransport) -> Self {
        Self {
            profile: DeviceProfile::baseline(kind),
            adapter: adapters::create(kind, transport),
            mutation: Mutex::new(()),
            settings_validation_policy: SettingsValidationPolicy::Permissive,
        }
    }

    /// Uses a distinct device profile while retaining the selected protocol adapter.
    #[must_use]
    pub fn with_device_definition(mut self, definition: DeviceDefinition) -> Self {
        self.profile = DeviceProfile::from_definition(definition);
        self
    }

    /// Transfers caller-supplied session secrets into this client only.
    #[must_use]
    pub fn with_session_credentials(mut self, credentials: HashMap<String, String>) -> Self {
        self.adapter.set_session_credentials(credentials);
        self
    }

    /// Selects validation strictness for dynamic settings.
    #[must_use]
    pub fn with_settings_validation_policy(mut self, policy: SettingsValidationPolicy) -> Self {
        self.settings_validation_policy = policy;
        self
    }

    #[must_use]
    pub fn profile(&self) -> &DeviceProfile {
        &self.profile
    }

    /// Verifies the connection through the selected adapter.
    ///
    /// # Errors
    ///
    /// Returns transport, HTTP, or device response errors.
    pub async fn verify_connection(&self) -> Result<(), SdkError> {
        self.adapter.verify_connection().await
    }

    /// Fetches the currently available information from the connected device.
    ///
    /// # Errors
    ///
    /// Returns `Unsupported` when the device has no information-page capability,
    /// or a transport, HTTP, or invalid-response error.
    pub async fn device_info(&self) -> Result<Vec<crate::DeviceInfoField>, SdkError> {
        self.require(ids::DEVICE_INFO)?;
        self.adapter.device_info().await
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
        self.adapter.list_files(&location).await
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
        match options.conflict_policy {
            ConflictPolicy::Fail => {}
            ConflictPolicy::OverwriteWhenSupported => {
                self.require(ids::UPLOAD_EXPLICIT_OVERWRITE)?;
            }
            ConflictPolicy::ReplaceWithBackup => self.require(ids::UPLOAD_BACKUP_REPLACE)?,
        }
        if options.prefer_websocket {
            self.require(ids::UPLOAD_WEBSOCKET)?;
        }
        let size = file_size(&local_path, "upload file").await?;
        let _guard = self.mutation.lock().await;
        self.adapter
            .upload(FileUpload {
                local_path,
                file_name,
                location,
                options,
                size,
                progress,
            })
            .await
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
        self.adapter.delete(&path).await
    }

    /// Deletes multiple remote files or empty directories.
    ///
    /// The adapter selects native batch deletion or serial execution.
    ///
    /// # Errors
    ///
    /// Returns validation, transport, protocol, or capability errors. A serial
    /// operation error reports how many paths completed before the failure.
    pub async fn delete_files(&self, paths: Vec<String>) -> Result<(), SdkError> {
        self.require(ids::FILE_DELETE)?;
        validate_batch_paths(&paths)?;
        let _guard = self.mutation.lock().await;
        self.adapter.delete_files(&paths).await
    }

    /// Streams a remote file to a local path.
    ///
    /// # Errors
    ///
    /// Returns file, transport, protocol, or capability errors.
    pub async fn download(&self, path: String, destination: PathBuf) -> Result<(), SdkError> {
        self.require(ids::FILE_DOWNLOAD)?;
        validate_remote_path(&path)?;

        self.adapter.download(&path, destination).await
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
                    &error,
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
        self.adapter.create_directory(&parent, &name).await
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
        self.adapter.rename(&path, &new_name).await
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
        self.adapter.move_file(&path, &destination).await
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
            if let Err(error) = self.adapter.move_file(path, &destination).await {
                return Err(batch_operation_error(
                    "move",
                    index,
                    paths.len(),
                    path,
                    &error,
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

        self.adapter.list_wifi_networks().await
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
        self.adapter.save_wifi_network(&credential).await
    }

    /// Deletes a saved Wi-Fi network.
    ///
    /// # Errors
    ///
    /// Returns validation, transport, protocol, or capability errors.
    pub async fn delete_wifi_network(&self, index: Option<u32>) -> Result<(), SdkError> {
        self.require(ids::WIFI_DELETE)?;

        let _guard = self.mutation.lock().await;
        self.adapter.delete_wifi_network(index).await
    }

    /// Lists images in the device wallpaper library.
    ///
    /// # Errors
    ///
    /// Returns capability, transport, or protocol errors.
    pub async fn list_wallpapers(&self) -> Result<Vec<FileEntry>, SdkError> {
        self.require(ids::WALLPAPERS_MANAGE)?;

        self.adapter.list_wallpapers().await
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
        self.upload_wallpaper_with_progress(
            local_path,
            file_name,
            overwrite,
            apply_to_lock_screen,
            None,
        )
        .await
    }

    /// Uploads an image and reports bytes supplied to the request body when supported.
    ///
    /// # Errors
    ///
    /// Returns validation, capability, file, transport, conflict, or protocol errors.
    pub async fn upload_wallpaper_with_progress(
        &self,
        local_path: PathBuf,
        file_name: String,
        overwrite: bool,
        apply_to_lock_screen: bool,
        progress: Option<Arc<dyn UploadProgressSink>>,
    ) -> Result<WallpaperUploadResult, SdkError> {
        self.require(ids::WALLPAPERS_UPLOAD)?;
        if progress.is_some() {
            self.require(ids::WALLPAPERS_UPLOAD_PROGRESS)?;
        }
        validate_file_name(&file_name)?;
        validate_extension(
            &file_name,
            &self.profile.file_formats.wallpaper_upload_extensions,
            "wallpaper image",
        )?;
        let size = file_size(&local_path, "wallpaper image").await?;
        let _guard = self.mutation.lock().await;
        self.adapter
            .upload_wallpaper(WallpaperUpload {
                local_path,
                file_name,
                overwrite,
                apply_to_lock_screen,
                size,
                progress,
            })
            .await
    }

    /// Deletes one image from the wallpaper library.
    ///
    /// # Errors
    ///
    /// Returns validation, capability, transport, or protocol errors.
    pub async fn delete_wallpaper(&self, file_name: String) -> Result<(), SdkError> {
        self.require(ids::WALLPAPERS_DELETE)?;
        validate_file_name(&file_name)?;
        validate_extension(
            &file_name,
            &self.profile.file_formats.wallpaper_upload_extensions,
            "wallpaper image",
        )?;
        let _guard = self.mutation.lock().await;
        self.adapter.delete_wallpaper(&file_name).await
    }

    /// Lists installed SD-card font families.
    /// # Errors
    /// Returns capability, transport, or protocol errors.
    pub async fn list_fonts(&self) -> Result<FontCatalog, SdkError> {
        self.require(ids::FONTS_LIST)?;

        self.adapter.list_fonts().await
    }

    /// Streams a device-supported font file without requesting replacement.
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

    /// Streams a font file with an explicit replacement preference.
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

    /// Validates and streams a font file to the connected device.
    ///
    /// # Errors
    ///
    /// Returns validation, capability, file, transport, or protocol errors.
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
        validate_extension(
            &file_name,
            &self.profile.file_formats.font_upload_extensions,
            "font file",
        )?;
        let size = file_size(&local_path, "font file").await?;
        let _guard = self.mutation.lock().await;
        self.adapter
            .upload_font(FontUpload {
                family,
                local_path,
                file_name,
                overwrite,
                size,
                progress,
            })
            .await
    }

    /// Deletes an installed font family.
    /// # Errors
    /// Returns validation, capability, transport, or protocol errors.
    pub async fn delete_font_family(&self, family: String) -> Result<(), SdkError> {
        self.require(ids::FONTS_DELETE)?;
        validate_file_name(&family)?;
        let _guard = self.mutation.lock().await;
        self.adapter.delete_font_family(&family).await
    }

    /// Lists saved OPDS servers without passwords.
    /// # Errors
    /// Returns capability, transport, or protocol errors.
    pub async fn list_opds_servers(&self) -> Result<Vec<OpdsServer>, SdkError> {
        self.require(ids::OPDS_LIST)?;

        self.adapter.list_opds_servers().await
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
        self.adapter.save_opds_server(&credential).await
    }

    /// Deletes a saved OPDS server by index.
    /// # Errors
    /// Returns capability, transport, or protocol errors.
    pub async fn delete_opds_server(&self, index: u32) -> Result<(), SdkError> {
        self.require(ids::OPDS_DELETE)?;

        let _guard = self.mutation.lock().await;
        self.adapter.delete_opds_server(index).await
    }

    /// Returns the current editable settings and metadata needed to render their controls.
    ///
    /// # Errors
    /// Returns capability, transport, or malformed response errors.
    pub async fn list_settings(&self) -> Result<SettingsSnapshot, SdkError> {
        self.require(ids::SETTINGS_LIST)?;

        self.adapter
            .list_settings(self.settings_validation_policy)
            .await
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
        self.adapter
            .apply_settings(expected, changes, self.settings_validation_policy)
            .await
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

fn validate_extension(name: &str, allowed: &[String], label: &str) -> Result<(), SdkError> {
    let extension = Path::new(name)
        .extension()
        .and_then(|v| v.to_str())
        .unwrap_or_default();
    if allowed.iter().any(|v| v.eq_ignore_ascii_case(extension)) {
        Ok(())
    } else {
        Err(SdkError::InvalidArgument(format!(
            "{label} extension is not supported by this device"
        )))
    }
}
async fn file_size(path: &Path, label: &str) -> Result<u64, SdkError> {
    let size = tokio::fs::metadata(path)
        .await
        .map_err(|e| SdkError::InvalidArgument(format!("cannot read {label}: {e}")))?
        .len();
    if size == 0 {
        Err(SdkError::InvalidArgument(format!(
            "{label} must not be empty"
        )))
    } else {
        Ok(size)
    }
}
