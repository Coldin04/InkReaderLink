//! Built-in device declarations and configurable device definitions.

use crate::{Capability, DeviceConstraints, DeviceFileFormats, DeviceKind, capability::ids};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceDefinition {
    pub device_type: String,
    pub capabilities: Vec<Capability>,
    pub constraints: DeviceConstraints,
    pub file_formats: DeviceFileFormats,
}

impl DeviceDefinition {
    #[must_use]
    pub fn upload_only(device_type: impl Into<String>, file_formats: DeviceFileFormats) -> Self {
        Self {
            device_type: device_type.into(),
            capabilities: vec![Capability::new(ids::FILE_UPLOAD)],
            constraints: DeviceConstraints {
                can_list_directories: false,
                can_choose_upload_directory: false,
            },
            file_formats,
        }
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
    }
}

#[must_use]
pub fn read_pico_definition() -> DeviceDefinition {
    DeviceDefinition::upload_only(
        "read-pico",
        DeviceFileFormats {
            accepts_any_upload_format: false,
            upload_extensions: extensions(&["epub", "txt"]),
            font_upload_extensions: Vec::new(),
            readable_extensions: extensions(&["epub", "txt"]),
        },
    )
    .with_capability(ids::DEVICE_INFO)
    .with_capability(ids::FILE_LIST)
    .with_capability(ids::FILE_DELETE)
    .with_capability(ids::UPLOAD_EXPLICIT_OVERWRITE)
    .with_capability(ids::WIFI_LIST)
    .with_capability(ids::WIFI_SAVE)
    .with_capability(ids::WIFI_DELETE)
}

#[must_use]
pub fn crosspoint_definition() -> DeviceDefinition {
    let mut definition = DeviceDefinition::upload_only(
        "crosspoint",
        DeviceFileFormats {
            accepts_any_upload_format: true,
            upload_extensions: Vec::new(),
            font_upload_extensions: extensions(&["cpfont"]),
            readable_extensions: extensions(&["epub", "txt", "md", "xtc"]),
        },
    )
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
                readable_extensions: vec!["epub".to_owned()],
            },
        );

        let capability_ids: Vec<_> = definition.capabilities.iter().map(Capability::id).collect();
        assert_eq!(capability_ids, [ids::FILE_UPLOAD]);
        assert!(!definition.constraints.can_list_directories);
    }
}
