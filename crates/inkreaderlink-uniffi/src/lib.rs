use std::{
    collections::HashSet,
    future::Future,
    path::PathBuf,
    sync::{Arc, OnceLock},
    time::Duration,
};

use inkreaderlink_core::{
    Capability, ConflictPolicy, DeviceClient, DeviceConnectionField, DeviceConnectionFieldKind,
    DeviceConstraints, DeviceFileFormats, DeviceInfoField, DeviceKind, DeviceProfile,
    DeviceResolution, FileDownload, FileEntry, FileKind, FileLocation, FontCatalog, FontFamily,
    FontFile, OpdsCredential, OpdsServer, SdkError, SettingChange, SettingDescriptor, SettingKind,
    SettingValue, SettingsSnapshot, UploadOptions, UploadProgressSink, UploadResult,
    WallpaperUploadResult, WifiCredential, WifiNetwork, built_in_definitions,
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
pub struct SdkFileDownload {
    pub path: String,
    pub destination: String,
}

impl From<SdkFileDownload> for FileDownload {
    fn from(file: SdkFileDownload) -> Self {
        Self {
            path: file.path,
            destination: PathBuf::from(file.destination),
        }
    }
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct SdkUploadResult {
    pub entry: SdkFileEntry,
    pub used_websocket: bool,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct SdkWallpaperUploadResult {
    pub entry: SdkFileEntry,
    pub applied_to_lock_screen: bool,
}

impl From<WallpaperUploadResult> for SdkWallpaperUploadResult {
    fn from(result: WallpaperUploadResult) -> Self {
        Self {
            entry: result.entry.into(),
            applied_to_lock_screen: result.applied_to_lock_screen,
        }
    }
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

#[derive(Clone, Debug, uniffi::Record)]
pub struct SdkFontFile {
    pub name: String,
    pub size: u64,
}

impl From<FontFile> for SdkFontFile {
    fn from(file: FontFile) -> Self {
        Self {
            name: file.name,
            size: file.size,
        }
    }
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct SdkFontFamily {
    pub name: String,
    pub sizes: Vec<u32>,
    pub files: Vec<SdkFontFile>,
}

impl From<FontFamily> for SdkFontFamily {
    fn from(family: FontFamily) -> Self {
        Self {
            name: family.name,
            sizes: family.sizes,
            files: family.files.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct SdkFontCatalog {
    pub max_families: u32,
    pub families: Vec<SdkFontFamily>,
}

impl From<FontCatalog> for SdkFontCatalog {
    fn from(catalog: FontCatalog) -> Self {
        Self {
            max_families: catalog.max_families,
            families: catalog.families.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct SdkOpdsServer {
    pub index: u32,
    pub name: String,
    pub url: String,
    pub username: String,
    pub has_password: bool,
}

impl From<OpdsServer> for SdkOpdsServer {
    fn from(server: OpdsServer) -> Self {
        Self {
            index: server.index,
            name: server.name,
            url: server.url,
            username: server.username,
            has_password: server.has_password,
        }
    }
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct SdkOpdsCredential {
    pub index: Option<u32>,
    pub name: String,
    pub url: String,
    pub username: String,
    pub password: Option<String>,
}

impl From<SdkOpdsCredential> for OpdsCredential {
    fn from(credential: SdkOpdsCredential) -> Self {
        Self {
            index: credential.index,
            name: credential.name,
            url: credential.url,
            username: credential.username,
            password: credential.password,
        }
    }
}

#[derive(Clone, Debug, uniffi::Enum)]
pub enum SdkSettingKind {
    Toggle,
    Choice { options: Vec<String> },
    Number { min: i64, max: i64, step: i64 },
    Text,
}

impl From<SettingKind> for SdkSettingKind {
    fn from(kind: SettingKind) -> Self {
        match kind {
            SettingKind::Toggle => Self::Toggle,
            SettingKind::Choice { options } => Self::Choice { options },
            SettingKind::Number { min, max, step } => Self::Number { min, max, step },
            SettingKind::Text => Self::Text,
        }
    }
}

impl From<SdkSettingKind> for SettingKind {
    fn from(kind: SdkSettingKind) -> Self {
        match kind {
            SdkSettingKind::Toggle => Self::Toggle,
            SdkSettingKind::Choice { options } => Self::Choice { options },
            SdkSettingKind::Number { min, max, step } => Self::Number { min, max, step },
            SdkSettingKind::Text => Self::Text,
        }
    }
}

#[derive(Clone, Debug, uniffi::Enum)]
pub enum SdkSettingValue {
    Toggle { value: bool },
    Choice { index: u32 },
    Number { value: i64 },
    Text { value: String },
}

impl From<SettingValue> for SdkSettingValue {
    fn from(value: SettingValue) -> Self {
        match value {
            SettingValue::Toggle(value) => Self::Toggle { value },
            SettingValue::Choice(index) => Self::Choice { index },
            SettingValue::Number(value) => Self::Number { value },
            SettingValue::Text(value) => Self::Text { value },
        }
    }
}

impl From<SdkSettingValue> for SettingValue {
    fn from(value: SdkSettingValue) -> Self {
        match value {
            SdkSettingValue::Toggle { value } => Self::Toggle(value),
            SdkSettingValue::Choice { index } => Self::Choice(index),
            SdkSettingValue::Number { value } => Self::Number(value),
            SdkSettingValue::Text { value } => Self::Text(value),
        }
    }
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct SdkSettingDescriptor {
    pub key: String,
    pub name: String,
    pub category: String,
    pub kind: SdkSettingKind,
    pub value: SdkSettingValue,
}

impl From<SettingDescriptor> for SdkSettingDescriptor {
    fn from(setting: SettingDescriptor) -> Self {
        Self {
            key: setting.key,
            name: setting.name,
            category: setting.category,
            kind: setting.kind.into(),
            value: setting.value.into(),
        }
    }
}

impl From<SdkSettingDescriptor> for SettingDescriptor {
    fn from(setting: SdkSettingDescriptor) -> Self {
        Self {
            key: setting.key,
            name: setting.name,
            category: setting.category,
            kind: setting.kind.into(),
            value: setting.value.into(),
        }
    }
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct SdkSettingsSnapshot {
    pub settings: Vec<SdkSettingDescriptor>,
}

impl From<SettingsSnapshot> for SdkSettingsSnapshot {
    fn from(snapshot: SettingsSnapshot) -> Self {
        Self {
            settings: snapshot.settings.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<SdkSettingsSnapshot> for SettingsSnapshot {
    fn from(snapshot: SdkSettingsSnapshot) -> Self {
        Self {
            settings: snapshot.settings.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct SdkSettingChange {
    pub key: String,
    pub value: SdkSettingValue,
}

impl From<SdkSettingChange> for SettingChange {
    fn from(change: SdkSettingChange) -> Self {
        Self {
            key: change.key,
            value: change.value.into(),
        }
    }
}

#[uniffi::export(foreign)]
pub trait SdkUploadProgressObserver: Send + Sync {
    fn on_progress(&self, sent_bytes: u64, total_bytes: u64);
}

struct FfiProgressSink {
    observer: Arc<dyn SdkUploadProgressObserver>,
}

impl UploadProgressSink for FfiProgressSink {
    fn report(&self, sent_bytes: u64, total_bytes: u64) {
        self.observer.on_progress(sent_bytes, total_bytes);
    }
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
    #[error("{detail}")]
    InvalidArgument { detail: String },
    #[error("{detail}")]
    Unsupported { detail: String },
    #[error("device unreachable")]
    Unreachable,
    #[error("operation timed out")]
    Timeout,
    #[error("{detail}")]
    Conflict { detail: String },
    #[error("insufficient storage")]
    InsufficientStorage,
    #[error("{detail}")]
    RemoteFailure { detail: String },
    #[error("{detail}")]
    CommittedWithWarning { detail: String },
    #[error("{detail}")]
    CommittedButCleanupFailed { detail: String },
    #[error("{detail}")]
    RecoveryFailed { detail: String },
}

impl From<SdkError> for SdkOperationError {
    fn from(error: SdkError) -> Self {
        match error {
            SdkError::InvalidArgument(detail) => Self::InvalidArgument { detail },
            SdkError::Unsupported(detail) => Self::Unsupported { detail },
            SdkError::Unreachable => Self::Unreachable,
            SdkError::Timeout => Self::Timeout,
            SdkError::Conflict(detail) => Self::Conflict { detail },
            SdkError::InsufficientStorage => Self::InsufficientStorage,
            SdkError::RemoteFailure(detail) => Self::RemoteFailure { detail },
            SdkError::CommittedWithWarning(detail) => Self::CommittedWithWarning { detail },
            SdkError::CommittedButCleanupFailed(detail) => {
                Self::CommittedButCleanupFailed { detail }
            }
            SdkError::RecoveryFailed(detail) => Self::RecoveryFailed { detail },
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
    pub display_resolution: Option<SdkDeviceResolution>,
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct SdkDeviceResolution {
    pub width: u32,
    pub height: u32,
}

impl From<DeviceResolution> for SdkDeviceResolution {
    fn from(resolution: DeviceResolution) -> Self {
        Self {
            width: resolution.width,
            height: resolution.height,
        }
    }
}

#[derive(Clone, Debug, uniffi::Enum)]
pub enum SdkConnectionFieldKind {
    Text,
    Address,
    Choice { options: Vec<String> },
    Toggle,
}

impl From<DeviceConnectionFieldKind> for SdkConnectionFieldKind {
    fn from(kind: DeviceConnectionFieldKind) -> Self {
        match kind {
            DeviceConnectionFieldKind::Text => Self::Text,
            DeviceConnectionFieldKind::Address => Self::Address,
            DeviceConnectionFieldKind::Choice { options } => Self::Choice { options },
            DeviceConnectionFieldKind::Toggle => Self::Toggle,
        }
    }
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct SdkConnectionField {
    pub key: String,
    pub label: String,
    pub kind: SdkConnectionFieldKind,
    pub required: bool,
}

#[derive(Clone, Debug, uniffi::Enum)]
pub enum SdkConnectionValue {
    Text { value: String },
    Address { value: String },
    Choice { index: u32 },
    Toggle { value: bool },
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct SdkConnectionParameter {
    pub key: String,
    pub value: SdkConnectionValue,
}

impl From<DeviceConnectionField> for SdkConnectionField {
    fn from(field: DeviceConnectionField) -> Self {
        Self {
            key: field.key,
            label: field.label,
            kind: field.kind.into(),
            required: field.required,
        }
    }
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct SdkSupportedDevice {
    pub device_type: String,
    pub display_name: String,
    pub connection_fields: Vec<SdkConnectionField>,
}

impl From<inkreaderlink_core::DeviceDefinition> for SdkSupportedDevice {
    fn from(definition: inkreaderlink_core::DeviceDefinition) -> Self {
        Self {
            device_type: definition.device_type,
            display_name: definition.display_name,
            connection_fields: definition
                .connection_fields
                .into_iter()
                .map(Into::into)
                .collect(),
        }
    }
}

/// Machine-readable item for a device information page. Localize `key` in the app.
#[derive(Clone, Debug, uniffi::Record)]
pub struct SdkDeviceInfoField {
    pub key: String,
    pub value: String,
}

impl From<DeviceInfoField> for SdkDeviceInfoField {
    fn from(field: DeviceInfoField) -> Self {
        Self {
            key: field.key,
            value: field.value,
        }
    }
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
    pub font_upload_extensions: Vec<String>,
    pub wallpaper_upload_extensions: Vec<String>,
    pub readable_extensions: Vec<String>,
}

impl From<DeviceFileFormats> for SdkDeviceFileFormats {
    fn from(formats: DeviceFileFormats) -> Self {
        Self {
            accepts_any_upload_format: formats.accepts_any_upload_format,
            upload_extensions: formats.upload_extensions,
            font_upload_extensions: formats.font_upload_extensions,
            wallpaper_upload_extensions: formats.wallpaper_upload_extensions,
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
            display_resolution: profile.display_resolution.map(Into::into),
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
            detail: format!("SDK task failed: {error}"),
        })?
        .map_err(Into::into)
}

fn resolve_connection_address(
    device_type: &str,
    parameters: Vec<SdkConnectionParameter>,
) -> Result<String, SdkOperationError> {
    let definition = built_in_definitions()
        .into_iter()
        .find(|definition| definition.device_type == device_type)
        .ok_or_else(|| SdkOperationError::Unsupported {
            detail: format!("unknown device type: {device_type}"),
        })?;
    let address_fields = definition
        .connection_fields
        .iter()
        .filter(|field| field.kind == DeviceConnectionFieldKind::Address)
        .count();
    if address_fields != 1 {
        return Err(SdkOperationError::InvalidArgument {
            detail: "device definition must declare exactly one address field".to_owned(),
        });
    }

    let mut seen = HashSet::new();
    let mut address = None;
    for parameter in parameters {
        if !seen.insert(parameter.key.clone()) {
            return Err(SdkOperationError::InvalidArgument {
                detail: format!("duplicate connection field: {}", parameter.key),
            });
        }
        let field = definition
            .connection_fields
            .iter()
            .find(|field| field.key == parameter.key)
            .ok_or_else(|| SdkOperationError::InvalidArgument {
                detail: format!("unknown connection field: {}", parameter.key),
            })?;
        let valid = match (&field.kind, &parameter.value) {
            (DeviceConnectionFieldKind::Text, SdkConnectionValue::Text { .. })
            | (DeviceConnectionFieldKind::Toggle, SdkConnectionValue::Toggle { .. }) => true,
            (DeviceConnectionFieldKind::Address, SdkConnectionValue::Address { value }) => {
                address = Some(value.clone());
                true
            }
            (
                DeviceConnectionFieldKind::Choice { options },
                SdkConnectionValue::Choice { index },
            ) => usize::try_from(*index).is_ok_and(|index| index < options.len()),
            _ => false,
        };
        if !valid {
            return Err(SdkOperationError::InvalidArgument {
                detail: format!("invalid value for connection field: {}", parameter.key),
            });
        }
    }

    for field in definition
        .connection_fields
        .iter()
        .filter(|field| field.required)
    {
        if !seen.contains(&field.key) {
            return Err(SdkOperationError::InvalidArgument {
                detail: format!("missing required connection field: {}", field.key),
            });
        }
    }

    address
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| SdkOperationError::InvalidArgument {
            detail: "device address is required".to_owned(),
        })
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
            "whiteos" => DeviceKind::WhiteOs,
            "wegooo-cell-fork" => DeviceKind::WegoCellFork,
            _ => {
                return Err(SdkOperationError::Unsupported {
                    detail: format!("unknown device type: {device_type}"),
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

    /// Connects only after the selected device's information endpoint is verified.
    ///
    /// # Errors
    ///
    /// Returns an error when the URL is invalid, the device cannot be reached, or its
    /// information response does not match the selected device type.
    #[uniffi::constructor]
    #[allow(clippy::needless_pass_by_value)]
    pub async fn connect_and_verify(
        device_type: String,
        base_url: String,
        timeout_ms: u64,
    ) -> Result<Arc<Self>, SdkOperationError> {
        let client = Self::connect(device_type, base_url, timeout_ms)?;
        let inner = Arc::clone(&client.inner);
        run_on_sdk_runtime(async move { inner.verify_connection().await }).await?;
        Ok(client)
    }

    /// Connects after validating the typed fields declared for the selected device.
    ///
    /// # Errors
    ///
    /// Returns an error for unknown, duplicate, missing, or mistyped fields, or when
    /// the device address cannot be reached or verified.
    #[uniffi::constructor]
    #[allow(clippy::needless_pass_by_value)]
    pub async fn connect_and_verify_with_parameters(
        device_type: String,
        parameters: Vec<SdkConnectionParameter>,
        timeout_ms: u64,
    ) -> Result<Arc<Self>, SdkOperationError> {
        let address = resolve_connection_address(&device_type, parameters)?;
        Self::connect_and_verify(device_type, address, timeout_ms).await
    }

    #[must_use]
    pub fn profile(&self) -> SdkDeviceProfile {
        self.inner.profile().clone().into()
    }

    /// Fetches device information for an information page. Keys are stable identifiers
    /// intended for app-side localization; values are returned as displayable text.
    ///
    /// # Errors
    ///
    /// Returns the mapped SDK operation error for unsupported capabilities,
    /// transport failures, or invalid device responses.
    pub async fn device_info(&self) -> Result<Vec<SdkDeviceInfoField>, SdkOperationError> {
        let inner = Arc::clone(&self.inner);
        run_on_sdk_runtime(async move {
            inner
                .device_info()
                .await
                .map(|fields| fields.into_iter().map(Into::into).collect())
        })
        .await
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
        progress_observer: Option<Arc<dyn SdkUploadProgressObserver>>,
    ) -> Result<SdkUploadResult, SdkOperationError> {
        let inner = Arc::clone(&self.inner);
        run_on_sdk_runtime(async move {
            inner
                .upload(
                    PathBuf::from(local_path),
                    file_name,
                    location.into(),
                    options.into(),
                    progress_observer.map(|observer| {
                        Arc::new(FfiProgressSink { observer }) as Arc<dyn UploadProgressSink>
                    }),
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

    /// Deletes multiple remote paths. Supported-device endpoint differences are
    /// handled by the SDK; sequential execution stops at the first failure.
    ///
    /// # Errors
    ///
    /// Returns validation, capability, transport, or protocol errors.
    pub async fn delete_files(&self, paths: Vec<String>) -> Result<(), SdkOperationError> {
        let inner = Arc::clone(&self.inner);
        run_on_sdk_runtime(async move { inner.delete_files(paths).await }).await
    }

    /// Lists images managed as wallpapers by the selected firmware.
    ///
    /// # Errors
    ///
    /// Returns capability, transport, or protocol errors.
    pub async fn list_wallpapers(&self) -> Result<Vec<SdkFileEntry>, SdkOperationError> {
        let inner = Arc::clone(&self.inner);
        run_on_sdk_runtime(async move {
            inner
                .list_wallpapers()
                .await
                .map(|entries| entries.into_iter().map(Into::into).collect())
        })
        .await
    }

    /// Uploads a wallpaper image and optionally applies it to the lock screen.
    ///
    /// # Errors
    ///
    /// Returns validation, capability, file, transport, conflict, or protocol errors.
    pub async fn upload_wallpaper(
        &self,
        local_path: String,
        file_name: String,
        overwrite: bool,
        apply_to_lock_screen: bool,
    ) -> Result<SdkWallpaperUploadResult, SdkOperationError> {
        let inner = Arc::clone(&self.inner);
        run_on_sdk_runtime(async move {
            inner
                .upload_wallpaper(
                    PathBuf::from(local_path),
                    file_name,
                    overwrite,
                    apply_to_lock_screen,
                )
                .await
                .map(Into::into)
        })
        .await
    }

    /// Deletes one wallpaper image by its file name.
    ///
    /// # Errors
    ///
    /// Returns validation, capability, transport, or protocol errors.
    pub async fn delete_wallpaper(&self, file_name: String) -> Result<(), SdkOperationError> {
        let inner = Arc::clone(&self.inner);
        run_on_sdk_runtime(async move { inner.delete_wallpaper(file_name).await }).await
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

    /// Downloads multiple remote files to the supplied local destinations.
    /// Downloads are streamed one at a time and stop at the first failure.
    ///
    /// # Errors
    ///
    /// Returns validation, file, capability, transport, or protocol errors.
    pub async fn download_files(
        &self,
        files: Vec<SdkFileDownload>,
    ) -> Result<(), SdkOperationError> {
        let inner = Arc::clone(&self.inner);
        let files = files.into_iter().map(Into::into).collect();
        run_on_sdk_runtime(async move { inner.download_files(files).await }).await
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

    /// Moves multiple remote files into one existing directory, serially.
    ///
    /// # Errors
    ///
    /// Returns validation, capability, transport, or protocol errors.
    pub async fn move_files(
        &self,
        paths: Vec<String>,
        destination: String,
    ) -> Result<(), SdkOperationError> {
        let inner = Arc::clone(&self.inner);
        run_on_sdk_runtime(async move { inner.move_files(paths, destination).await }).await
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

    /// Lists installed SD-card font families.
    /// # Errors
    /// Returns capability, transport, or protocol errors.
    pub async fn list_fonts(&self) -> Result<SdkFontCatalog, SdkOperationError> {
        let inner = Arc::clone(&self.inner);
        run_on_sdk_runtime(async move { inner.list_fonts().await.map(Into::into) }).await
    }

    /// Streams a device-supported font file. Read Pico rejects same-name files by default.
    /// # Errors
    /// Returns validation, capability, file, transport, or protocol errors.
    pub async fn upload_font(
        &self,
        family: String,
        local_path: String,
        file_name: String,
    ) -> Result<(), SdkOperationError> {
        let inner = Arc::clone(&self.inner);
        run_on_sdk_runtime(async move {
            inner
                .upload_font(family, PathBuf::from(local_path), file_name)
                .await
        })
        .await
    }

    /// Streams a font file and reports bytes supplied to the HTTP request body.
    /// # Errors
    /// Returns validation, capability, file, transport, or protocol errors.
    pub async fn upload_font_with_progress(
        &self,
        family: String,
        local_path: String,
        file_name: String,
        progress_observer: Arc<dyn SdkUploadProgressObserver>,
    ) -> Result<(), SdkOperationError> {
        let inner = Arc::clone(&self.inner);
        run_on_sdk_runtime(async move {
            inner
                .upload_font_with_progress(
                    family,
                    PathBuf::from(local_path),
                    file_name,
                    Arc::new(FfiProgressSink {
                        observer: progress_observer,
                    }),
                )
                .await
        })
        .await
    }

    /// Streams a font file with progress and an explicit overwrite choice.
    /// # Errors
    /// Returns validation, capability, file, transport, or protocol errors.
    pub async fn upload_font_with_overwrite_and_progress(
        &self,
        family: String,
        local_path: String,
        file_name: String,
        overwrite: bool,
        progress_observer: Arc<dyn SdkUploadProgressObserver>,
    ) -> Result<(), SdkOperationError> {
        let inner = Arc::clone(&self.inner);
        run_on_sdk_runtime(async move {
            inner
                .upload_font_with_overwrite_and_progress(
                    family,
                    PathBuf::from(local_path),
                    file_name,
                    overwrite,
                    Some(Arc::new(FfiProgressSink {
                        observer: progress_observer,
                    })),
                )
                .await
        })
        .await
    }

    /// Streams a font and optionally replaces a same-name Read Pico font.
    /// # Errors
    /// Returns validation, capability, file, transport, or protocol errors.
    pub async fn upload_font_with_overwrite(
        &self,
        family: String,
        local_path: String,
        file_name: String,
        overwrite: bool,
    ) -> Result<(), SdkOperationError> {
        let inner = Arc::clone(&self.inner);
        run_on_sdk_runtime(async move {
            inner
                .upload_font_with_overwrite(family, PathBuf::from(local_path), file_name, overwrite)
                .await
        })
        .await
    }

    /// Deletes an installed font family.
    /// # Errors
    /// Returns validation, capability, transport, or protocol errors.
    pub async fn delete_font_family(&self, family: String) -> Result<(), SdkOperationError> {
        let inner = Arc::clone(&self.inner);
        run_on_sdk_runtime(async move { inner.delete_font_family(family).await }).await
    }

    /// Lists saved OPDS servers without passwords.
    /// # Errors
    /// Returns capability, transport, or protocol errors.
    pub async fn list_opds_servers(&self) -> Result<Vec<SdkOpdsServer>, SdkOperationError> {
        let inner = Arc::clone(&self.inner);
        run_on_sdk_runtime(async move {
            inner
                .list_opds_servers()
                .await
                .map(|servers| servers.into_iter().map(Into::into).collect())
        })
        .await
    }

    /// Adds or updates an OPDS server.
    /// # Errors
    /// Returns validation, capability, transport, or protocol errors.
    pub async fn save_opds_server(
        &self,
        credential: SdkOpdsCredential,
    ) -> Result<(), SdkOperationError> {
        let inner = Arc::clone(&self.inner);
        run_on_sdk_runtime(async move { inner.save_opds_server(credential.into()).await }).await
    }

    /// Deletes an OPDS server by index.
    /// # Errors
    /// Returns capability, transport, or protocol errors.
    pub async fn delete_opds_server(&self, index: u32) -> Result<(), SdkOperationError> {
        let inner = Arc::clone(&self.inner);
        run_on_sdk_runtime(async move { inner.delete_opds_server(index).await }).await
    }

    /// Returns current editable settings and rendering metadata.
    /// # Errors
    /// Returns capability, transport, or protocol errors.
    pub async fn list_settings(&self) -> Result<SdkSettingsSnapshot, SdkOperationError> {
        let inner = Arc::clone(&self.inner);
        run_on_sdk_runtime(async move { inner.list_settings().await.map(Into::into) }).await
    }

    /// Validates and applies changes against a previously returned snapshot.
    /// # Errors
    /// Returns capability, conflict, validation, transport, or protocol errors.
    pub async fn apply_settings(
        &self,
        expected: SdkSettingsSnapshot,
        changes: Vec<SdkSettingChange>,
    ) -> Result<SdkSettingsSnapshot, SdkOperationError> {
        let inner = Arc::clone(&self.inner);
        run_on_sdk_runtime(async move {
            inner
                .apply_settings(
                    expected.into(),
                    changes.into_iter().map(Into::into).collect(),
                )
                .await
                .map(Into::into)
        })
        .await
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

    /// Lists SDK-supported devices and the fields required to configure each connection.
    #[must_use]
    pub fn supported_devices(&self) -> Vec<SdkSupportedDevice> {
        built_in_definitions().into_iter().map(Into::into).collect()
    }

    #[must_use]
    #[allow(clippy::needless_pass_by_value)]
    pub fn baseline_profile(&self, device_type: String) -> Option<SdkDeviceProfile> {
        let kind = match device_type.as_str() {
            "read-pico" => DeviceKind::ReadPico,
            "crosspoint" => DeviceKind::CrossPoint,
            "whiteos" => DeviceKind::WhiteOs,
            "wegooo-cell-fork" => DeviceKind::WegoCellFork,
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
