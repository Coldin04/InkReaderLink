use std::{
    future::Future,
    path::PathBuf,
    sync::{Arc, OnceLock},
    time::Duration,
};

use booksend_core::{
    Capability, ConflictPolicy, DeviceClient, DeviceConstraints, DeviceFileFormats, DeviceKind,
    DeviceProfile, FileEntry, FileKind, FileLocation, SdkError, UploadOptions, UploadResult,
    WifiCredential, WifiNetwork,
};

#[derive(Clone, Debug, uniffi::Enum)]
pub enum SdkConflictPolicy {
    Fail,
    OverwriteWhenSupported,
    ReplaceWithBackup,
}

impl From<SdkConflictPolicy> for ConflictPolicy {
    fn from(policy: SdkConflictPolicy) -> Self {
        match policy {
            SdkConflictPolicy::Fail => Self::Fail,
            SdkConflictPolicy::OverwriteWhenSupported => Self::OverwriteWhenSupported,
            SdkConflictPolicy::ReplaceWithBackup => Self::ReplaceWithBackup,
        }
    }
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct SdkUploadOptions {
    pub conflict_policy: SdkConflictPolicy,
    pub content_type: Option<String>,
    pub prefer_websocket: bool,
}

impl From<SdkUploadOptions> for UploadOptions {
    fn from(options: SdkUploadOptions) -> Self {
        Self {
            conflict_policy: options.conflict_policy.into(),
            content_type: options.content_type,
            prefer_websocket: options.prefer_websocket,
        }
    }
}

#[derive(Clone, Debug, uniffi::Enum)]
pub enum SdkFileKind {
    Book,
    Directory,
    Other,
}

impl From<FileKind> for SdkFileKind {
    fn from(kind: FileKind) -> Self {
        match kind {
            FileKind::Book => Self::Book,
            FileKind::Directory => Self::Directory,
            FileKind::Other => Self::Other,
        }
    }
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct SdkFileEntry {
    pub name: String,
    pub path: String,
    pub size: u64,
    pub kind: SdkFileKind,
}

impl From<FileEntry> for SdkFileEntry {
    fn from(entry: FileEntry) -> Self {
        Self {
            name: entry.name,
            path: entry.path,
            size: entry.size,
            kind: entry.kind.into(),
        }
    }
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct SdkUploadResult {
    pub entry: SdkFileEntry,
    pub used_websocket: bool,
}

impl From<UploadResult> for SdkUploadResult {
    fn from(result: UploadResult) -> Self {
        Self {
            entry: result.entry.into(),
            used_websocket: result.used_websocket,
        }
    }
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct SdkWifiNetwork {
    pub index: Option<u32>,
    pub ssid: String,
    pub has_password: bool,
    pub is_last_connected: bool,
}

impl From<WifiNetwork> for SdkWifiNetwork {
    fn from(network: WifiNetwork) -> Self {
        Self {
            index: network.index,
            ssid: network.ssid,
            has_password: network.has_password,
            is_last_connected: network.is_last_connected,
        }
    }
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct SdkWifiCredential {
    pub index: Option<u32>,
    pub ssid: String,
    pub password: Option<String>,
}

impl From<SdkWifiCredential> for WifiCredential {
    fn from(credential: SdkWifiCredential) -> Self {
        Self {
            index: credential.index,
            ssid: credential.ssid,
            password: credential.password,
        }
    }
}

#[derive(Clone, Debug, uniffi::Error, thiserror::Error)]
pub enum SdkOperationError {
    #[error("{message}")]
    InvalidArgument { message: String },
    #[error("{message}")]
    Unsupported { message: String },
    #[error("device unreachable")]
    Unreachable,
    #[error("operation timed out")]
    Timeout,
    #[error("{message}")]
    Conflict { message: String },
    #[error("insufficient storage")]
    InsufficientStorage,
    #[error("{message}")]
    RemoteFailure { message: String },
    #[error("{message}")]
    CommittedWithWarning { message: String },
    #[error("{message}")]
    CommittedButCleanupFailed { message: String },
    #[error("{message}")]
    RecoveryFailed { message: String },
}

impl From<SdkError> for SdkOperationError {
    fn from(error: SdkError) -> Self {
        match error {
            SdkError::InvalidArgument(message) => Self::InvalidArgument { message },
            SdkError::Unsupported(message) => Self::Unsupported { message },
            SdkError::Unreachable => Self::Unreachable,
            SdkError::Timeout => Self::Timeout,
            SdkError::Conflict(message) => Self::Conflict { message },
            SdkError::InsufficientStorage => Self::InsufficientStorage,
            SdkError::RemoteFailure(message) => Self::RemoteFailure { message },
            SdkError::CommittedWithWarning(message) => Self::CommittedWithWarning { message },
            SdkError::CommittedButCleanupFailed(message) => {
                Self::CommittedButCleanupFailed { message }
            }
            SdkError::RecoveryFailed(message) => Self::RecoveryFailed { message },
        }
    }
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct SdkDeviceIdentity {
    pub device_type: String,
    pub device_id: Option<String>,
    pub firmware_version: Option<String>,
    pub protocol_version: Option<u32>,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct SdkDeviceProfile {
    pub identity: SdkDeviceIdentity,
    pub capabilities: Vec<String>,
    pub constraints: SdkDeviceConstraints,
    pub file_formats: SdkDeviceFileFormats,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct SdkDeviceConstraints {
    pub can_list_directories: bool,
    pub can_choose_upload_directory: bool,
}

impl From<DeviceConstraints> for SdkDeviceConstraints {
    fn from(constraints: DeviceConstraints) -> Self {
        Self {
            can_list_directories: constraints.can_list_directories,
            can_choose_upload_directory: constraints.can_choose_upload_directory,
        }
    }
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct SdkDeviceFileFormats {
    pub accepts_any_upload_format: bool,
    pub upload_extensions: Vec<String>,
    pub readable_extensions: Vec<String>,
}

impl From<DeviceFileFormats> for SdkDeviceFileFormats {
    fn from(formats: DeviceFileFormats) -> Self {
        Self {
            accepts_any_upload_format: formats.accepts_any_upload_format,
            upload_extensions: formats.upload_extensions,
            readable_extensions: formats.readable_extensions,
        }
    }
}

impl From<DeviceProfile> for SdkDeviceProfile {
    fn from(profile: DeviceProfile) -> Self {
        Self {
            identity: SdkDeviceIdentity {
                device_type: profile.identity.device_type,
                device_id: profile.identity.device_id,
                firmware_version: profile.identity.firmware_version,
                protocol_version: profile.identity.protocol_version,
            },
            capabilities: profile
                .capabilities
                .iter()
                .map(Capability::id)
                .map(str::to_owned)
                .collect(),
            constraints: profile.constraints.into(),
            file_formats: profile.file_formats.into(),
        }
    }
}

#[derive(Clone, Debug, uniffi::Enum)]
pub enum SdkFileLocation {
    Root,
    Directory { path: String },
}

impl From<SdkFileLocation> for FileLocation {
    fn from(location: SdkFileLocation) -> Self {
        match location {
            SdkFileLocation::Root => Self::Root,
            SdkFileLocation::Directory { path } => Self::Directory(path),
        }
    }
}

#[derive(Debug, uniffi::Object)]
pub struct BooksendSdk;

#[derive(Debug, uniffi::Object)]
pub struct SdkDeviceClient {
    inner: Arc<DeviceClient>,
}

fn sdk_runtime() -> &'static tokio::runtime::Runtime {
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("SDK Tokio runtime must be constructible")
    })
}

async fn run_on_sdk_runtime<T, F>(future: F) -> Result<T, SdkOperationError>
where
    T: Send + 'static,
    F: Future<Output = Result<T, SdkError>> + Send + 'static,
{
    sdk_runtime()
        .spawn(future)
        .await
        .map_err(|error| SdkOperationError::RemoteFailure {
            message: format!("SDK task failed: {error}"),
        })?
        .map_err(Into::into)
}

#[uniffi::export]
impl SdkDeviceClient {
    /// Connects a typed device client to an HTTP origin.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown device type or invalid URL.
    #[uniffi::constructor]
    #[allow(clippy::needless_pass_by_value)]
    pub fn connect(
        device_type: String,
        base_url: String,
        timeout_ms: u64,
    ) -> Result<Arc<Self>, SdkOperationError> {
        let kind = match device_type.as_str() {
            "read-pico" => DeviceKind::ReadPico,
            "crosspoint" => DeviceKind::CrossPoint,
            _ => {
                return Err(SdkOperationError::Unsupported {
                    message: format!("unknown device type: {device_type}"),
                });
            }
        };
        let inner =
            DeviceClient::connect(kind, &base_url, Duration::from_millis(timeout_ms.max(1)))
                .map_err(SdkOperationError::from)?;
        Ok(Arc::new(Self {
            inner: Arc::new(inner),
        }))
    }

    #[must_use]
    pub fn profile(&self) -> SdkDeviceProfile {
        self.inner.profile().clone().into()
    }

    /// Lists files at a device location.
    ///
    /// # Errors
    ///
    /// Returns capability, transport, or protocol errors.
    pub async fn list_files(
        &self,
        location: SdkFileLocation,
    ) -> Result<Vec<SdkFileEntry>, SdkOperationError> {
        let inner = Arc::clone(&self.inner);
        run_on_sdk_runtime(async move {
            inner
                .list_files(location.into())
                .await
                .map(|entries| entries.into_iter().map(Into::into).collect())
        })
        .await
    }

    /// Streams a local file to the device.
    ///
    /// # Errors
    ///
    /// Returns validation, file, capability, transport, or protocol errors.
    pub async fn upload(
        &self,
        local_path: String,
        file_name: String,
        location: SdkFileLocation,
        options: SdkUploadOptions,
    ) -> Result<SdkUploadResult, SdkOperationError> {
        let inner = Arc::clone(&self.inner);
        run_on_sdk_runtime(async move {
            inner
                .upload(
                    PathBuf::from(local_path),
                    file_name,
                    location.into(),
                    options.into(),
                )
                .await
                .map(Into::into)
        })
        .await
    }

    /// Deletes a remote file or empty directory.
    ///
    /// # Errors
    ///
    /// Returns validation, capability, transport, or protocol errors.
    pub async fn delete(&self, path: String) -> Result<(), SdkOperationError> {
        let inner = Arc::clone(&self.inner);
        run_on_sdk_runtime(async move { inner.delete(path).await }).await
    }

    /// Streams a remote file to a local destination.
    ///
    /// # Errors
    ///
    /// Returns file, capability, transport, or protocol errors.
    pub async fn download(
        &self,
        path: String,
        destination: String,
    ) -> Result<(), SdkOperationError> {
        let inner = Arc::clone(&self.inner);
        run_on_sdk_runtime(async move { inner.download(path, PathBuf::from(destination)).await })
            .await
    }

    /// Creates a remote directory.
    ///
    /// # Errors
    ///
    /// Returns validation, capability, transport, or protocol errors.
    pub async fn create_directory(
        &self,
        parent: String,
        name: String,
    ) -> Result<(), SdkOperationError> {
        let inner = Arc::clone(&self.inner);
        run_on_sdk_runtime(async move { inner.create_directory(parent, name).await }).await
    }

    /// Renames a remote file.
    ///
    /// # Errors
    ///
    /// Returns validation, capability, transport, or protocol errors.
    pub async fn rename(&self, path: String, new_name: String) -> Result<(), SdkOperationError> {
        let inner = Arc::clone(&self.inner);
        run_on_sdk_runtime(async move { inner.rename(path, new_name).await }).await
    }

    /// Moves a remote file.
    ///
    /// # Errors
    ///
    /// Returns validation, capability, transport, or protocol errors.
    pub async fn move_file(
        &self,
        path: String,
        destination: String,
    ) -> Result<(), SdkOperationError> {
        let inner = Arc::clone(&self.inner);
        run_on_sdk_runtime(async move { inner.move_file(path, destination).await }).await
    }

    /// Lists saved Wi-Fi credentials without passwords.
    ///
    /// # Errors
    ///
    /// Returns capability, transport, or protocol errors.
    pub async fn list_wifi_networks(&self) -> Result<Vec<SdkWifiNetwork>, SdkOperationError> {
        let inner = Arc::clone(&self.inner);
        run_on_sdk_runtime(async move {
            inner
                .list_wifi_networks()
                .await
                .map(|networks| networks.into_iter().map(Into::into).collect())
        })
        .await
    }

    /// Adds or updates a Wi-Fi credential.
    ///
    /// # Errors
    ///
    /// Returns validation, capability, transport, or protocol errors.
    pub async fn save_wifi_network(
        &self,
        credential: SdkWifiCredential,
    ) -> Result<(), SdkOperationError> {
        let inner = Arc::clone(&self.inner);
        run_on_sdk_runtime(async move { inner.save_wifi_network(credential.into()).await }).await
    }

    /// Deletes a saved Wi-Fi credential.
    ///
    /// # Errors
    ///
    /// Returns validation, capability, transport, or protocol errors.
    pub async fn delete_wifi_network(&self, index: Option<u32>) -> Result<(), SdkOperationError> {
        let inner = Arc::clone(&self.inner);
        run_on_sdk_runtime(async move { inner.delete_wifi_network(index).await }).await
    }
}

#[uniffi::export]
impl BooksendSdk {
    #[uniffi::constructor]
    #[must_use]
    pub fn new() -> Arc<Self> {
        Arc::new(Self)
    }

    #[must_use]
    pub fn version(&self) -> String {
        env!("CARGO_PKG_VERSION").to_owned()
    }

    #[must_use]
    #[allow(clippy::needless_pass_by_value)]
    pub fn baseline_profile(&self, device_type: String) -> Option<SdkDeviceProfile> {
        let kind = match device_type.as_str() {
            "read-pico" => DeviceKind::ReadPico,
            "crosspoint" => DeviceKind::CrossPoint,
            _ => return None,
        };
        Some(DeviceProfile::baseline(kind).into())
    }
}

uniffi::setup_scaffolding!();

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_extensible_string_capabilities() {
        let sdk = BooksendSdk::new();
        let profile = sdk.baseline_profile("read-pico".to_owned()).unwrap();

        assert_eq!(profile.identity.device_type, "read-pico");
        assert!(profile.capabilities.contains(&"files.upload".to_owned()));
    }
}
