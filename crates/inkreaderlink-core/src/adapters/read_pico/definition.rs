use crate::config::extensions;
use crate::{
    ConnectionFieldRetention, DeviceConnectionField, DeviceConnectionFieldKind, DeviceDefinition,
    DeviceFileFormats, capability::ids,
};
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
        retention: ConnectionFieldRetention::AppManaged,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DeviceKind, DeviceProfile};
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
}
