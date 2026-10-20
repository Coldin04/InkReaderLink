use crate::{SdkConnectionParameter, SdkConnectionValue, SdkOperationError};
use inkreaderlink_core::{DeviceConnectionFieldKind, built_in_definitions};
use std::collections::{HashMap, HashSet};
pub(crate) struct ResolvedConnection {
    pub(crate) address: String,
    pub(crate) session_credentials: HashMap<String, String>,
}

pub(crate) fn resolve_builtin_device_definition(
    device_type: &str,
) -> Result<inkreaderlink_core::DeviceDefinition, SdkOperationError> {
    built_in_definitions()
        .into_iter()
        .find(|definition| definition.device_type == device_type)
        .ok_or_else(|| SdkOperationError::Unsupported {
            detail: format!("unknown device type: {device_type}"),
        })
}

pub(crate) fn resolve_connection_parameters(
    definition: &inkreaderlink_core::DeviceDefinition,
    parameters: Vec<SdkConnectionParameter>,
) -> Result<ResolvedConnection, SdkOperationError> {
    let address_fields = definition
        .connection_fields
        .iter()
        .filter(|field| field.kind == DeviceConnectionFieldKind::Address)
        .count();
    if address_fields != 1 {
        return Err(SdkOperationError::InvalidArgument {
            detail: "device definition must declare exactly one address field".to_owned(),
        });
    }

    let mut seen = HashSet::new();
    let mut address = None;
    let mut session_credentials = HashMap::new();
    for parameter in parameters {
        if !seen.insert(parameter.key.clone()) {
            return Err(SdkOperationError::InvalidArgument {
                detail: format!("duplicate connection field: {}", parameter.key),
            });
        }
        let field = definition
            .connection_fields
            .iter()
            .find(|field| field.key == parameter.key)
            .ok_or_else(|| SdkOperationError::InvalidArgument {
                detail: format!("unknown connection field: {}", parameter.key),
            })?;
        let valid = match (&field.kind, &parameter.value) {
            (DeviceConnectionFieldKind::Text, SdkConnectionValue::Text { .. })
            | (DeviceConnectionFieldKind::Toggle, SdkConnectionValue::Toggle { .. }) => true,
            (DeviceConnectionFieldKind::Secret, SdkConnectionValue::Secret { value }) => {
                if !value.is_empty() {
                    session_credentials.insert(parameter.key.clone(), value.clone());
                }
                true
            }
            (DeviceConnectionFieldKind::Address, SdkConnectionValue::Address { value }) => {
                address = Some(value.clone());
                true
            }
            (
                DeviceConnectionFieldKind::Choice { options },
                SdkConnectionValue::Choice { index },
            ) => usize::try_from(*index).is_ok_and(|index| index < options.len()),
            _ => false,
        };
        if !valid {
            return Err(SdkOperationError::InvalidArgument {
                detail: format!("invalid value for connection field: {}", parameter.key),
            });
        }
    }

    for field in definition
        .connection_fields
        .iter()
        .filter(|field| field.required)
    {
        if !seen.contains(&field.key) {
            return Err(SdkOperationError::InvalidArgument {
                detail: format!("missing required connection field: {}", field.key),
            });
        }
    }

    let address = address
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| SdkOperationError::InvalidArgument {
            detail: "device address is required".to_owned(),
        })?;
    Ok(ResolvedConnection {
        address,
        session_credentials,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use inkreaderlink_core::{
        ConnectionFieldRetention, DeviceConnectionField, DeviceDefinition, DeviceFileFormats,
    };
    fn definition() -> DeviceDefinition {
        DeviceDefinition::upload_only(
            "test-reader",
            DeviceFileFormats {
                accepts_any_upload_format: true,
                upload_extensions: vec![],
                font_upload_extensions: vec![],
                wallpaper_upload_extensions: vec![],
                readable_extensions: vec![],
            },
        )
        .with_connection_field(DeviceConnectionField {
            key: "endpoint".to_owned(),
            label: "Address".to_owned(),
            kind: DeviceConnectionFieldKind::Address,
            required: true,
            retention: ConnectionFieldRetention::AppManaged,
        })
        .with_connection_field(DeviceConnectionField {
            key: "secret".to_owned(),
            label: "Secret".to_owned(),
            kind: DeviceConnectionFieldKind::Secret,
            required: false,
            retention: ConnectionFieldRetention::Session,
        })
    }
    fn address() -> SdkConnectionParameter {
        SdkConnectionParameter {
            key: "endpoint".to_owned(),
            value: SdkConnectionValue::Address {
                value: "reader.example.test".to_owned(),
            },
        }
    }
    #[test]
    fn maps_declared_address_and_optional_session_secret() {
        let definition = definition();
        let probe = resolve_connection_parameters(&definition, vec![address()]).unwrap();
        assert_eq!(probe.address, "reader.example.test");
        assert!(probe.session_credentials.is_empty());
        let secret = SdkConnectionParameter {
            key: "secret".to_owned(),
            value: SdkConnectionValue::Secret {
                value: "test-secret".to_owned(),
            },
        };
        let linked =
            resolve_connection_parameters(&definition, vec![address(), secret.clone()]).unwrap();
        assert_eq!(linked.session_credentials["secret"], "test-secret");
        assert!(!format!("{secret:?}").contains("test-secret"));
    }
    #[test]
    fn rejects_missing_duplicate_unknown_and_mistyped_fields() {
        let definition = definition();
        for parameters in [
            vec![],
            vec![address(), address()],
            vec![
                address(),
                SdkConnectionParameter {
                    key: "unknown".to_owned(),
                    value: SdkConnectionValue::Toggle { value: true },
                },
            ],
            vec![SdkConnectionParameter {
                key: "endpoint".to_owned(),
                value: SdkConnectionValue::Text {
                    value: "reader.example.test".to_owned(),
                },
            }],
        ] {
            assert!(matches!(
                resolve_connection_parameters(&definition, parameters),
                Err(SdkOperationError::InvalidArgument { .. })
            ));
        }
    }
}
