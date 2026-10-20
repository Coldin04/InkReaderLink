use crate::config::extensions;
use crate::{
    ConnectionFieldRetention, DeviceConnectionField, DeviceConnectionFieldKind, DeviceDefinition,
    DeviceFileFormats, capability::ids,
};
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
        retention: ConnectionFieldRetention::AppManaged,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Capability, DeviceKind, DeviceProfile};
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
}
