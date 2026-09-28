use crate::{Capability, capability::ids as capability};

pub const READ_PICO_DEVICE_TYPE: &str = "read-pico";
pub const CROSSPOINT_DEVICE_TYPE: &str = "crosspoint";

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
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceProfile {
    pub identity: DeviceIdentity,
    pub capabilities: Vec<Capability>,
}

#[must_use]
pub fn route_device(device_type: &str) -> Option<DeviceKind> {
    match device_type {
        READ_PICO_DEVICE_TYPE => Some(DeviceKind::ReadPico),
        CROSSPOINT_DEVICE_TYPE => Some(DeviceKind::CrossPoint),
        _ => None,
    }
}

impl DeviceProfile {
    #[must_use]
    pub fn baseline(kind: DeviceKind) -> Self {
        let (device_type, capability_ids): (&str, &[&str]) = match kind {
            DeviceKind::ReadPico => (
                READ_PICO_DEVICE_TYPE,
                &[
                    capability::FILE_LIST,
                    capability::FILE_UPLOAD,
                    capability::FILE_DELETE,
                    capability::UPLOAD_EXPLICIT_OVERWRITE,
                    capability::WIFI_MANAGE,
                ],
            ),
            DeviceKind::CrossPoint => (
                CROSSPOINT_DEVICE_TYPE,
                &[
                    capability::FILE_LIST,
                    capability::FILE_UPLOAD,
                    capability::FILE_DELETE,
                    capability::FILE_DOWNLOAD,
                    capability::FILE_RENAME,
                    capability::FILE_MOVE,
                    capability::DIRECTORY_CREATE,
                    capability::UPLOAD_WEBSOCKET,
                    capability::WIFI_MANAGE,
                ],
            ),
        };

        Self {
            identity: DeviceIdentity {
                device_type: device_type.to_owned(),
                device_id: None,
                firmware_version: None,
                protocol_version: None,
            },
            capabilities: capability_ids
                .iter()
                .map(|id| Capability::new(*id))
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

        assert!(ids.contains(&capability::FILE_UPLOAD));
        assert!(ids.contains(&capability::FILE_RENAME));
        assert!(ids.contains(&capability::UPLOAD_WEBSOCKET));
    }
}
