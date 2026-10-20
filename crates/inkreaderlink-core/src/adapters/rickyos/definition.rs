use crate::DeviceDefinition;
use crate::config::crosspoint_definition;
use crate::device::RICKYOS_DEVICE_TYPE;
#[must_use]
pub fn rickyos_definition() -> DeviceDefinition {
    let mut definition = crosspoint_definition();
    RICKYOS_DEVICE_TYPE.clone_into(&mut definition.device_type);
    "RickyOS".clone_into(&mut definition.display_name);
    definition
}
