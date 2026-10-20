use crate::config::extensions;
use crate::{
    ConnectionFieldRetention, DeviceConnectionField, DeviceConnectionFieldKind, DeviceDefinition,
    DeviceFileFormats,
};
#[must_use]
pub fn pico_page_definition() -> DeviceDefinition {
    DeviceDefinition::upload_only(
        "pico-page",
        DeviceFileFormats {
            accepts_any_upload_format: false,
            upload_extensions: extensions(&["epub", "txt", "ttf", "otf", "png", "jpg", "jpeg"]),
            font_upload_extensions: Vec::new(),
            wallpaper_upload_extensions: Vec::new(),
            readable_extensions: extensions(&["epub", "txt"]),
        },
    )
    .with_display_name("Pico Page")
    .with_connection_field(DeviceConnectionField {
        key: "address".to_owned(),
        label: "设备地址".to_owned(),
        kind: DeviceConnectionFieldKind::Address,
        required: true,
        retention: ConnectionFieldRetention::AppManaged,
    })
    .with_connection_field(DeviceConnectionField {
        key: "access_code".to_owned(),
        label: "设备访问码".to_owned(),
        kind: DeviceConnectionFieldKind::Secret,
        required: false,
        retention: ConnectionFieldRetention::Session,
    })
}
