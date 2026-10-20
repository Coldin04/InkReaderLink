use crate::DeviceDefinition;
use crate::config::crosspoint_definition;
use crate::device::CROSSMUX_DEVICE_TYPE;
#[must_use]
pub fn crossmux_definition() -> DeviceDefinition {
    let mut definition = crosspoint_definition();
    CROSSMUX_DEVICE_TYPE.clone_into(&mut definition.device_type);
    "CrossMux".clone_into(&mut definition.display_name);
    definition
}
