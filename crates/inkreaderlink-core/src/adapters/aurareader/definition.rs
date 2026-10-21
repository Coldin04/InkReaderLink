use crate::config::extensions;
use crate::device::AURAREADER_DEVICE_TYPE;
use crate::{
    ConnectionFieldRetention, DeviceConnectionField, DeviceConnectionFieldKind, DeviceDefinition,
    DeviceFileFormats, capability::ids,
};

#[must_use]
pub fn aurareader_definition() -> DeviceDefinition {
    DeviceDefinition::upload_only(
        AURAREADER_DEVICE_TYPE,
        DeviceFileFormats {
            accepts_any_upload_format: false,
            upload_extensions: extensions(&["txt", "epub"]),
            font_upload_extensions: extensions(&["ttf"]),
            wallpaper_upload_extensions: Vec::new(),
            readable_extensions: extensions(&["txt", "epub"]),
        },
    )
    .with_display_name("AuraReader")
    .with_connection_field(DeviceConnectionField {
        key: "address".into(),
        label: "设备地址".into(),
        kind: DeviceConnectionFieldKind::Address,
        required: true,
        retention: ConnectionFieldRetention::AppManaged,
    })
    .with_capability(ids::DEVICE_INFO)
    .with_capability(ids::FILE_LIST)
    .with_capability(ids::FILE_DELETE)
    .with_capability(ids::UPLOAD_EXPLICIT_OVERWRITE)
    .with_capability(ids::FONTS_UPLOAD)
    .with_capability(ids::FONTS_UPLOAD_PROGRESS)
    .with_capability(ids::WIFI_LIST)
    .with_capability(ids::WIFI_SAVE)
    .with_capability(ids::WIFI_DELETE)
    .with_capability(ids::SETTINGS_LIST)
    .with_capability(ids::SETTINGS_UPDATE)
}
