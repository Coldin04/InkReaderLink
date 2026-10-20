use crate::config::extensions;
use crate::device::PAGEREST_DEVICE_TYPE;
use crate::{
    ConnectionFieldRetention, DeviceConnectionField, DeviceConnectionFieldKind, DeviceDefinition,
    DeviceFileFormats, capability::ids,
};
#[must_use]
pub fn pagerest_definition() -> DeviceDefinition {
    DeviceDefinition::upload_only(
        PAGEREST_DEVICE_TYPE,
        DeviceFileFormats {
            accepts_any_upload_format: false,
            upload_extensions: extensions(&["epub", "txt"]),
            font_upload_extensions: extensions(&["ttf"]),
            wallpaper_upload_extensions: extensions(&["jpg", "jpeg", "png"]),
            readable_extensions: extensions(&["epub", "txt"]),
        },
    )
    .with_display_name("PageRest")
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
    .with_capability(ids::FONTS_LIST)
    .with_capability(ids::FONTS_UPLOAD)
    .with_capability(ids::FONTS_UPLOAD_PROGRESS)
    .with_capability(ids::FONTS_DELETE)
    .with_capability(ids::WALLPAPERS_UPLOAD)
    .with_capability(ids::WALLPAPERS_UPLOAD_PROGRESS)
    .with_capability(ids::WALLPAPERS_MANAGE)
    .with_capability(ids::WALLPAPERS_DELETE)
    .with_capability(ids::WIFI_LIST)
    .with_capability(ids::WIFI_SAVE)
    .with_capability(ids::WIFI_DELETE)
}
