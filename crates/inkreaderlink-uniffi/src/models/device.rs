use inkreaderlink_core::{
    Capability, DeviceConstraints, DeviceFileFormats, DeviceInfoField, DeviceProfile,
    DeviceResolution,
};

use crate::SdkConnectionField;
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
