use crate::{
    Capability, DeviceConstraints, DeviceDefinition, DeviceFileFormats, DeviceResolution,
    built_in_definition,
};

pub const READ_PICO_DEVICE_TYPE: &str = "read-pico";
pub const CROSSPOINT_DEVICE_TYPE: &str = "crosspoint";
pub const CROSSMUX_DEVICE_TYPE: &str = "crossmux";
pub const RICKYOS_DEVICE_TYPE: &str = "rickyos";
pub const IF_INK_OS_DEVICE_TYPE: &str = "if-ink-os";
pub const WHITEOS_DEVICE_TYPE: &str = "whiteos";
pub const KIIKOREAD_DEVICE_TYPE: &str = "kiikoread";
pub const JUSTREAD_DEVICE_TYPE: &str = "justread";
pub const PICO_PAGE_DEVICE_TYPE: &str = "pico-page";
pub const PAGEREST_DEVICE_TYPE: &str = "pagerest";
pub const FOLIOBOOK_DEVICE_TYPE: &str = "foliobook";

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
    KiikoRead,
    JustRead,
    PicoPage,
    IfInkOs,
    PageRest,
    FolioBook,
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
        CROSSPOINT_DEVICE_TYPE | CROSSMUX_DEVICE_TYPE | RICKYOS_DEVICE_TYPE => {
            Some(DeviceKind::CrossPoint)
        }
        IF_INK_OS_DEVICE_TYPE => Some(DeviceKind::IfInkOs),
        WHITEOS_DEVICE_TYPE => Some(DeviceKind::WhiteOs),
        KIIKOREAD_DEVICE_TYPE => Some(DeviceKind::KiikoRead),
        JUSTREAD_DEVICE_TYPE => Some(DeviceKind::JustRead),
        PICO_PAGE_DEVICE_TYPE => Some(DeviceKind::PicoPage),
        PAGEREST_DEVICE_TYPE => Some(DeviceKind::PageRest),
        FOLIOBOOK_DEVICE_TYPE => Some(DeviceKind::FolioBook),
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
    fn profile_restores_the_required_upload_capability() {
        let mut definition = built_in_definition(DeviceKind::ReadPico);
        definition.capabilities.clear();

        let profile = DeviceProfile::from_definition(definition);

        assert_eq!(profile.capabilities.len(), 1);
        assert_eq!(profile.capabilities[0].id(), ids::FILE_UPLOAD);
    }
}
