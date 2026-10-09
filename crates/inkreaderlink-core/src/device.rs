use crate::{
    Capability, DeviceConstraints, DeviceDefinition, DeviceFileFormats, DeviceResolution,
    built_in_definition,
};

pub const READ_PICO_DEVICE_TYPE: &str = "read-pico";
pub const CROSSPOINT_DEVICE_TYPE: &str = "crosspoint";
pub const WHITEOS_DEVICE_TYPE: &str = "whiteos";
pub const WEGOOO_CELL_FORK_DEVICE_TYPE: &str = "wegooo-cell-fork";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceIdentity {
    pub device_type: String,
    pub device_id: Option<String>,
    pub firmware_version: Option<String>,
    pub protocol_version: Option<u32>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DeviceKind {
    ReadPico,
    CrossPoint,
    WhiteOs,
    WegoCellFork,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceProfile {
    pub identity: DeviceIdentity,
    pub capabilities: Vec<Capability>,
    pub constraints: DeviceConstraints,
    pub file_formats: DeviceFileFormats,
    pub display_resolution: Option<DeviceResolution>,
}

#[must_use]
pub fn route_device(device_type: &str) -> Option<DeviceKind> {
    match device_type {
        READ_PICO_DEVICE_TYPE => Some(DeviceKind::ReadPico),
        CROSSPOINT_DEVICE_TYPE => Some(DeviceKind::CrossPoint),
        WHITEOS_DEVICE_TYPE => Some(DeviceKind::WhiteOs),
        WEGOOO_CELL_FORK_DEVICE_TYPE => Some(DeviceKind::WegoCellFork),
        _ => None,
    }
}

impl DeviceProfile {
    #[must_use]
    pub fn baseline(kind: DeviceKind) -> Self {
        Self::from_definition(built_in_definition(kind))
    }

    #[must_use]
    pub fn from_definition(mut definition: DeviceDefinition) -> Self {
        definition.ensure_upload_capability();
        Self {
            identity: DeviceIdentity {
                device_type: definition.device_type,
                device_id: None,
                firmware_version: None,
                protocol_version: None,
            },
            capabilities: definition.capabilities,
            constraints: definition.constraints,
            file_formats: definition.file_formats,
            display_resolution: definition.display_resolution,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::ids;

    #[test]
    fn routes_known_devices_without_exposing_adapter_types() {
        assert_eq!(route_device("read-pico"), Some(DeviceKind::ReadPico));
        assert_eq!(route_device("crosspoint"), Some(DeviceKind::CrossPoint));
        assert_eq!(route_device("future-reader"), None);
    }

    #[test]
    fn crosspoint_declares_optional_file_management_capabilities() {
        let profile = DeviceProfile::baseline(DeviceKind::CrossPoint);
        let ids: Vec<_> = profile.capabilities.iter().map(Capability::id).collect();

        assert!(ids.contains(&ids::FILE_UPLOAD));
        assert!(ids.contains(&ids::FILE_RENAME));
        assert!(ids.contains(&ids::UPLOAD_WEBSOCKET));
        assert!(ids.contains(&ids::FONTS_LIST));
        assert!(ids.contains(&ids::FONTS_UPLOAD));
        assert!(ids.contains(&ids::FONTS_DELETE));
        assert!(ids.contains(&ids::OPDS_LIST));
        assert!(ids.contains(&ids::OPDS_SAVE));
        assert!(ids.contains(&ids::OPDS_DELETE));
        assert!(ids.contains(&ids::SETTINGS_LIST));
        assert!(ids.contains(&ids::SETTINGS_UPDATE));
        assert!(profile.constraints.can_list_directories);
        assert!(profile.constraints.can_choose_upload_directory);
        assert!(profile.file_formats.accepts_any_upload_format);
        assert_eq!(profile.file_formats.font_upload_extensions, ["cpfont"]);
        assert!(
            profile
                .file_formats
                .readable_extensions
                .contains(&"xtc".to_owned())
        );
    }

    #[test]
    fn read_pico_restricts_file_operations_to_its_active_root() {
        let profile = DeviceProfile::baseline(DeviceKind::ReadPico);

        assert!(!profile.constraints.can_list_directories);
        assert!(!profile.constraints.can_choose_upload_directory);
        assert!(!profile.file_formats.accepts_any_upload_format);
        assert_eq!(profile.file_formats.upload_extensions, ["epub", "txt"]);
        assert_eq!(profile.file_formats.font_upload_extensions, ["ttf"]);
        assert!(
            profile
                .capabilities
                .iter()
                .any(|capability| capability.id() == ids::FONTS_UPLOAD)
        );
        assert!(
            !profile
                .capabilities
                .iter()
                .any(|capability| capability.id() == ids::FONTS_LIST)
        );
        assert!(
            !profile
                .capabilities
                .iter()
                .any(|capability| capability.id() == ids::OPDS_LIST)
        );
        assert!(
            !profile
                .capabilities
                .iter()
                .any(|capability| capability.id() == ids::SETTINGS_LIST)
        );
    }

    #[test]
    fn profile_restores_the_required_upload_capability() {
        let mut definition = built_in_definition(DeviceKind::ReadPico);
        definition.capabilities.clear();

        let profile = DeviceProfile::from_definition(definition);

        assert_eq!(profile.capabilities.len(), 1);
        assert_eq!(profile.capabilities[0].id(), ids::FILE_UPLOAD);
    }
}
