mod adapters;
pub mod capability;
mod device;

pub use capability::Capability;
pub use device::{DeviceIdentity, DeviceKind, DeviceProfile, route_device};
