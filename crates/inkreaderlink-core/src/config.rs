//! Built-in device declarations and configurable device definitions.

use crate::{
    Capability, DeviceConnectionField, DeviceConstraints, DeviceFileFormats, DeviceKind,
    DeviceResolution, capability::ids,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeviceDefinition {
    pub device_type: String,
    pub display_name: String,
    pub connection_fields: Vec<DeviceConnectionField>,
    pub capabilities: Vec<Capability>,
    pub constraints: DeviceConstraints,
    pub file_formats: DeviceFileFormats,
    pub display_resolution: Option<DeviceResolution>,
}

impl DeviceDefinition {
    #[must_use]
    pub fn upload_only(device_type: impl Into<String>, file_formats: DeviceFileFormats) -> Self {
        Self {
            device_type: device_type.into(),
            display_name: String::new(),
            connection_fields: Vec::new(),
            capabilities: vec![Capability::new(ids::FILE_UPLOAD)],
            constraints: DeviceConstraints {
                can_list_directories: false,
                can_choose_upload_directory: false,
            },
            file_formats,
            display_resolution: None,
        }
    }

    #[must_use]
    pub fn with_display_name(mut self, display_name: impl Into<String>) -> Self {
        self.display_name = display_name.into();
        self
    }

    #[must_use]
    pub fn with_display_resolution(mut self, width: u32, height: u32) -> Self {
        self.display_resolution = Some(DeviceResolution { width, height });
        self
    }

    #[must_use]
    pub fn with_connection_field(mut self, field: DeviceConnectionField) -> Self {
        self.connection_fields.push(field);
        self
    }

    #[must_use]
    pub fn with_capability(mut self, capability: impl Into<String>) -> Self {
        let capability = capability.into();
        if !self.capabilities.iter().any(|item| item.id() == capability) {
            self.capabilities.push(Capability::new(capability));
        }
        self
    }

    pub(crate) fn ensure_upload_capability(&mut self) {
        if !self
            .capabilities
            .iter()
            .any(|item| item.id() == ids::FILE_UPLOAD)
        {
            self.capabilities.push(Capability::new(ids::FILE_UPLOAD));
        }
    }
}

#[must_use]
pub fn built_in_definition(kind: DeviceKind) -> DeviceDefinition {
    match kind {
        DeviceKind::ReadPico => read_pico_definition(),
        DeviceKind::CrossPoint => crosspoint_definition(),
        DeviceKind::WhiteOs => whiteos_definition(),
        DeviceKind::KiikoRead => kiikoread_definition(),
        DeviceKind::JustRead => justread_definition(),
        DeviceKind::PicoPage => pico_page_definition(),
        DeviceKind::IfInkOs => if_ink_os_definition(),
        DeviceKind::PageRest => pagerest_definition(),
    }
}

#[must_use]
pub fn built_in_definitions() -> Vec<DeviceDefinition> {
    vec![
        justread_definition(),
        read_pico_definition(),
        crosspoint_definition(),
        crossmux_definition(),
        rickyos_definition(),
        if_ink_os_definition(),
        whiteos_definition(),
        kiikoread_definition(),
        pico_page_definition(),
        pagerest_definition(),
    ]
}

pub use crate::adapters::crossmux::definition::crossmux_definition;
pub use crate::adapters::crosspoint::definition::crosspoint_definition;
pub use crate::adapters::if_ink_os::definition::if_ink_os_definition;
pub use crate::adapters::justread::definition::justread_definition;
pub use crate::adapters::kiikoread::definition::kiikoread_definition;
pub use crate::adapters::pagerest::definition::pagerest_definition;
pub use crate::adapters::pico_page::definition::pico_page_definition;
pub use crate::adapters::read_pico::definition::read_pico_definition;
pub use crate::adapters::rickyos::definition::rickyos_definition;
pub use crate::adapters::whiteos::definition::whiteos_definition;

pub(crate) fn extensions(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upload_only_definition_has_no_optional_capabilities() {
        let definition = DeviceDefinition::upload_only(
            "minimal-reader",
            DeviceFileFormats {
                accepts_any_upload_format: false,
                upload_extensions: vec!["epub".to_owned()],
                font_upload_extensions: Vec::new(),
                wallpaper_upload_extensions: Vec::new(),
                readable_extensions: vec!["epub".to_owned()],
            },
        );

        let capability_ids: Vec<_> = definition.capabilities.iter().map(Capability::id).collect();
        assert_eq!(capability_ids, [ids::FILE_UPLOAD]);
        assert!(!definition.constraints.can_list_directories);
    }
}
