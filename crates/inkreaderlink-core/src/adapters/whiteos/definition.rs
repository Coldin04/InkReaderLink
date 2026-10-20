use crate::config::extensions;
use crate::{
    ConnectionFieldRetention, DeviceConnectionField, DeviceConnectionFieldKind, DeviceDefinition,
    DeviceFileFormats, capability::ids,
};
#[must_use]
pub fn whiteos_definition() -> DeviceDefinition {
    let mut definition = DeviceDefinition::upload_only(
        "whiteos",
        DeviceFileFormats {
            accepts_any_upload_format: true,
            upload_extensions: Vec::new(),
            font_upload_extensions: Vec::new(),
            wallpaper_upload_extensions: Vec::new(),
            readable_extensions: extensions(&["txt", "epub", "pdf"]),
        },
    )
    .with_display_name("whiteos")
    .with_connection_field(DeviceConnectionField {
        key: "address".to_owned(),
        label: "设备地址".to_owned(),
        kind: DeviceConnectionFieldKind::Address,
        required: true,
        retention: ConnectionFieldRetention::AppManaged,
    })
    .with_capability(ids::FILE_LIST)
    .with_capability(ids::FILE_DIRECTORY_LIST)
    .with_capability(ids::UPLOAD_DIRECTORY_TARGET)
    .with_capability(ids::FILE_DELETE)
    .with_capability(ids::FILE_DOWNLOAD)
    .with_capability(ids::FILE_RENAME)
    .with_capability(ids::DIRECTORY_CREATE);
    definition.constraints.can_list_directories = true;
    definition.constraints.can_choose_upload_directory = true;
    definition
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Capability;
    #[test]
    fn whiteos_definition_declares_its_file_management_capabilities() {
        let definition = whiteos_definition();
        let capability_ids: Vec<_> = definition.capabilities.iter().map(Capability::id).collect();

        assert_eq!(definition.device_type, "whiteos");
        assert_eq!(definition.display_name, "whiteos");
        for required in [
            ids::FILE_UPLOAD,
            ids::FILE_LIST,
            ids::FILE_DIRECTORY_LIST,
            ids::UPLOAD_DIRECTORY_TARGET,
            ids::FILE_DELETE,
            ids::FILE_DOWNLOAD,
            ids::FILE_RENAME,
            ids::DIRECTORY_CREATE,
        ] {
            assert!(capability_ids.contains(&required));
        }
        assert!(definition.file_formats.accepts_any_upload_format);
        assert_eq!(
            definition.file_formats.upload_extensions,
            Vec::<String>::new()
        );
        assert_eq!(
            definition.file_formats.readable_extensions,
            vec!["txt".to_owned(), "epub".to_owned(), "pdf".to_owned()]
        );
    }
}
