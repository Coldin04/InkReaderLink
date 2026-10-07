//! Built-in device declarations and configurable device definitions.

use crate::{
    Capability, DeviceConnectionField, DeviceConnectionFieldKind, DeviceConstraints,
    DeviceFileFormats, DeviceKind, DeviceResolution, capability::ids,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceDefinition {
    pub device_type: String,
    pub display_name: String,
    pub connection_fields: Vec<DeviceConnectionField>,
    pub capabilities: Vec<Capability>,
    pub constraints: DeviceConstraints,
    pub file_formats: DeviceFileFormats,
    pub display_resolution: Option<DeviceResolution>,
}

impl DeviceDefinition {
    #[must_use]
    pub fn upload_only(device_type: impl Into<String>, file_formats: DeviceFileFormats) -> Self {
        Self {
            device_type: device_type.into(),
            display_name: String::new(),
            connection_fields: Vec::new(),
            capabilities: vec![Capability::new(ids::FILE_UPLOAD)],
            constraints: DeviceConstraints {
                can_list_directories: false,
                can_choose_upload_directory: false,
            },
            file_formats,
            display_resolution: None,
        }
    }

    #[must_use]
    pub fn with_display_name(mut self, display_name: impl Into<String>) -> Self {
        self.display_name = display_name.into();
        self
    }

    #[must_use]
    pub fn with_display_resolution(mut self, width: u32, height: u32) -> Self {
        self.display_resolution = Some(DeviceResolution { width, height });
        self
    }

    #[must_use]
    pub fn with_connection_field(mut self, field: DeviceConnectionField) -> Self {
        self.connection_fields.push(field);
        self
    }

    #[must_use]
    pub fn with_capability(mut self, capability: impl Into<String>) -> Self {
        let capability = capability.into();
        if !self.capabilities.iter().any(|item| item.id() == capability) {
            self.capabilities.push(Capability::new(capability));
        }
        self
    }

    pub(crate) fn ensure_upload_capability(&mut self) {
        if !self
            .capabilities
            .iter()
            .any(|item| item.id() == ids::FILE_UPLOAD)
        {
            self.capabilities.push(Capability::new(ids::FILE_UPLOAD));
        }
    }
}

#[must_use]
pub fn built_in_definition(kind: DeviceKind) -> DeviceDefinition {
    match kind {
        DeviceKind::ReadPico => read_pico_definition(),
        DeviceKind::CrossPoint => crosspoint_definition(),
        DeviceKind::WegoCellFork => wegooo_cell_fork_definition(),
    }
}

#[must_use]
pub fn built_in_definitions() -> Vec<DeviceDefinition> {
    vec![
        read_pico_definition(),
        crosspoint_definition(),
        wegooo_cell_fork_definition(),
    ]
}

#[must_use]
pub fn read_pico_definition() -> DeviceDefinition {
    DeviceDefinition::upload_only(
        "read-pico",
        DeviceFileFormats {
            accepts_any_upload_format: false,
            upload_extensions: extensions(&["epub", "txt"]),
            font_upload_extensions: extensions(&["ttf"]),
            wallpaper_upload_extensions: Vec::new(),
            readable_extensions: extensions(&["epub", "txt"]),
        },
    )
    .with_display_name("Read Pico")
    .with_display_resolution(684, 1216)
    .with_connection_field(DeviceConnectionField {
        key: "address".to_owned(),
        label: "设备地址".to_owned(),
        kind: DeviceConnectionFieldKind::Address,
        required: true,
    })
    .with_capability(ids::DEVICE_INFO)
    .with_capability(ids::FILE_LIST)
    .with_capability(ids::FILE_DELETE)
    .with_capability(ids::UPLOAD_EXPLICIT_OVERWRITE)
    .with_capability(ids::FONTS_UPLOAD_PROGRESS)
    .with_capability(ids::WIFI_LIST)
    .with_capability(ids::WIFI_SAVE)
    .with_capability(ids::WIFI_DELETE)
    .with_capability(ids::FONTS_UPLOAD)
}

#[must_use]
pub fn crosspoint_definition() -> DeviceDefinition {
    let mut definition = DeviceDefinition::upload_only(
        "crosspoint",
        DeviceFileFormats {
            accepts_any_upload_format: true,
            upload_extensions: Vec::new(),
            font_upload_extensions: extensions(&["cpfont"]),
            wallpaper_upload_extensions: Vec::new(),
            readable_extensions: extensions(&["epub", "txt", "md", "xtc"]),
        },
    )
    .with_display_name("CrossPoint")
    .with_connection_field(DeviceConnectionField {
        key: "address".to_owned(),
        label: "设备地址".to_owned(),
        kind: DeviceConnectionFieldKind::Address,
        required: true,
    })
    .with_capability(ids::DEVICE_INFO)
    .with_capability(ids::FILE_LIST)
    .with_capability(ids::FILE_DIRECTORY_LIST)
    .with_capability(ids::UPLOAD_DIRECTORY_TARGET)
    .with_capability(ids::FILE_DELETE)
    .with_capability(ids::FILE_DOWNLOAD)
    .with_capability(ids::FILE_RENAME)
    .with_capability(ids::FILE_MOVE)
    .with_capability(ids::DIRECTORY_CREATE)
    .with_capability(ids::UPLOAD_BACKUP_REPLACE)
    .with_capability(ids::UPLOAD_WEBSOCKET)
    .with_capability(ids::WIFI_LIST)
    .with_capability(ids::WIFI_SAVE)
    .with_capability(ids::WIFI_DELETE)
    .with_capability(ids::FONTS_LIST)
    .with_capability(ids::FONTS_UPLOAD)
    .with_capability(ids::FONTS_UPLOAD_FAMILY)
    .with_capability(ids::FONTS_DELETE)
    .with_capability(ids::OPDS_LIST)
    .with_capability(ids::OPDS_SAVE)
    .with_capability(ids::OPDS_DELETE)
    .with_capability(ids::SETTINGS_LIST)
    .with_capability(ids::SETTINGS_UPDATE);
    definition.constraints.can_list_directories = true;
    definition.constraints.can_choose_upload_directory = true;
    definition
}

#[must_use]
pub fn wegooo_cell_fork_definition() -> DeviceDefinition {
    let mut definition = DeviceDefinition::upload_only(
        "wegooo-cell-fork",
        DeviceFileFormats {
            accepts_any_upload_format: false,
            upload_extensions: extensions(&["epub", "txt"]),
            font_upload_extensions: extensions(&["ttf", "otf"]),
            wallpaper_upload_extensions: extensions(&["jpg", "jpeg", "png"]),
            readable_extensions: extensions(&["epub", "txt"]),
        },
    )
    .with_display_name("kiiko 厂长 Fork固件")
    .with_display_resolution(684, 1216)
    .with_connection_field(DeviceConnectionField {
        key: "address".to_owned(),
        label: "设备地址".to_owned(),
        kind: DeviceConnectionFieldKind::Address,
        required: true,
    })
    .with_capability(ids::DEVICE_INFO)
    .with_capability(ids::FILE_LIST)
    .with_capability(ids::FILE_DIRECTORY_LIST)
    .with_capability(ids::FILE_DELETE)
    .with_capability(ids::UPLOAD_EXPLICIT_OVERWRITE)
    .with_capability(ids::FONTS_UPLOAD)
    .with_capability(ids::WIFI_LIST)
    .with_capability(ids::WIFI_SAVE)
    .with_capability(ids::WIFI_DELETE)
    .with_capability(ids::WALLPAPERS_UPLOAD)
    .with_capability(ids::WALLPAPERS_MANAGE)
    .with_capability(ids::WALLPAPERS_DELETE);
    definition.constraints.can_list_directories = true;
    definition
}

fn extensions(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upload_only_definition_has_no_optional_capabilities() {
        let definition = DeviceDefinition::upload_only(
            "minimal-reader",
            DeviceFileFormats {
                accepts_any_upload_format: false,
                upload_extensions: vec!["epub".to_owned()],
                font_upload_extensions: Vec::new(),
                wallpaper_upload_extensions: Vec::new(),
                readable_extensions: vec!["epub".to_owned()],
            },
        );

        let capability_ids: Vec<_> = definition.capabilities.iter().map(Capability::id).collect();
        assert_eq!(capability_ids, [ids::FILE_UPLOAD]);
        assert!(!definition.constraints.can_list_directories);
    }
}
