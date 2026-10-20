use crate::{
    ConnectionFieldRetention, DeviceConnectionField, DeviceConnectionFieldKind, DeviceDefinition,
    DeviceFileFormats,
};

#[must_use]
pub fn foliobook_definition() -> DeviceDefinition {
    DeviceDefinition::upload_only(
        "foliobook",
        DeviceFileFormats {
            // The supplied browser uploader has no file-type restriction.
            accepts_any_upload_format: true,
            upload_extensions: Vec::new(),
            font_upload_extensions: Vec::new(),
            wallpaper_upload_extensions: Vec::new(),
            // The upload page does not document the reader's native formats.
            readable_extensions: Vec::new(),
        },
    )
    .with_display_name("FolioBook")
    .with_connection_field(DeviceConnectionField {
        key: "address".to_owned(),
        label: "设备地址".to_owned(),
        kind: DeviceConnectionFieldKind::Address,
        required: true,
        retention: ConnectionFieldRetention::AppManaged,
    })
}
