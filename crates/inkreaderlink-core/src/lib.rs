pub mod adapters;
pub mod capability;
mod client;
pub mod config;
mod device;
pub mod model;
pub mod settings;
pub mod transport;
mod validation;

pub use capability::Capability;
pub use client::DeviceClient;
pub use config::{DeviceDefinition, built_in_definition, built_in_definitions};
pub use device::{DeviceIdentity, DeviceKind, DeviceProfile, route_device};
pub use model::{
    ConflictPolicy, ConnectionFieldRetention, DeviceConnectionField, DeviceConnectionFieldKind,
    DeviceConstraints, DeviceFileFormats, DeviceInfoField, DeviceResolution, FileDownload,
    FileEntry, FileKind, FileLocation, FontCatalog, FontFamily, FontFile, OpdsCredential,
    OpdsServer, SdkError, SettingChange, SettingDescriptor, SettingKind, SettingValue,
    SettingsSnapshot, SettingsValidationPolicy, UploadOptions, UploadProgressSink, UploadResult,
    WallpaperUploadResult, WifiCredential, WifiNetwork,
};

#[cfg(test)]
mod client_tests;

#[cfg(test)]
mod test_support;
