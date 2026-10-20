use crate::config::extensions;
use crate::{
    ConnectionFieldRetention, DeviceConnectionField, DeviceConnectionFieldKind, DeviceDefinition,
    DeviceFileFormats, capability::ids,
};
#[must_use]
pub fn justread_definition() -> DeviceDefinition {
    let mut definition = DeviceDefinition::upload_only(
        "justread",
        DeviceFileFormats {
            accepts_any_upload_format: true,
            upload_extensions: Vec::new(),
            font_upload_extensions: Vec::new(),
            wallpaper_upload_extensions: Vec::new(),
            readable_extensions: extensions(&["epub", "txt"]),
        },
    )
    .with_display_name("JustRead")
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
    .with_capability(ids::FILE_LIST)
    .with_capability(ids::FILE_DIRECTORY_LIST)
    .with_capability(ids::UPLOAD_DIRECTORY_TARGET)
    .with_capability(ids::FILE_DOWNLOAD);
    definition.constraints.can_list_directories = true;
    definition.constraints.can_choose_upload_directory = true;
    definition
}
