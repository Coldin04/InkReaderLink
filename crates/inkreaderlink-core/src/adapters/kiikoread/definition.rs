use crate::config::extensions;
use crate::{
    ConnectionFieldRetention, DeviceConnectionField, DeviceConnectionFieldKind, DeviceDefinition,
    DeviceFileFormats, capability::ids,
};
#[must_use]
pub fn kiikoread_definition() -> DeviceDefinition {
    let mut definition = DeviceDefinition::upload_only(
        "kiikoread",
        DeviceFileFormats {
            accepts_any_upload_format: false,
            upload_extensions: extensions(&["epub", "txt"]),
            font_upload_extensions: extensions(&["ttf", "otf"]),
            wallpaper_upload_extensions: extensions(&["jpg", "jpeg", "png"]),
            readable_extensions: extensions(&["epub", "txt"]),
        },
    )
    .with_display_name("KiikoRead")
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
