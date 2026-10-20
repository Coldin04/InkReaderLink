use inkreaderlink_core::{
    ConnectionFieldRetention, DeviceConnectionField, DeviceConnectionFieldKind,
};

#[derive(Clone, Debug, uniffi::Enum)]
pub enum SdkConnectionFieldKind {
    Text,
    Secret,
    Address,
    Choice { options: Vec<String> },
    Toggle,
}

impl From<DeviceConnectionFieldKind> for SdkConnectionFieldKind {
    fn from(kind: DeviceConnectionFieldKind) -> Self {
        match kind {
            DeviceConnectionFieldKind::Text => Self::Text,
            DeviceConnectionFieldKind::Secret => Self::Secret,
            DeviceConnectionFieldKind::Address => Self::Address,
            DeviceConnectionFieldKind::Choice { options } => Self::Choice { options },
            DeviceConnectionFieldKind::Toggle => Self::Toggle,
        }
    }
}

#[derive(Clone, Debug, uniffi::Enum)]
pub enum SdkConnectionFieldRetention {
    AppManaged,
    Session,
}

impl From<ConnectionFieldRetention> for SdkConnectionFieldRetention {
    fn from(retention: ConnectionFieldRetention) -> Self {
        match retention {
            ConnectionFieldRetention::AppManaged => Self::AppManaged,
            ConnectionFieldRetention::Session => Self::Session,
        }
    }
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct SdkConnectionField {
    pub key: String,
    pub label: String,
    pub kind: SdkConnectionFieldKind,
    pub required: bool,
    pub retention: SdkConnectionFieldRetention,
}

#[derive(Clone, uniffi::Enum)]
pub enum SdkConnectionValue {
    Text { value: String },
    Secret { value: String },
    Address { value: String },
    Choice { index: u32 },
    Toggle { value: bool },
}

impl std::fmt::Debug for SdkConnectionValue {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Text { value } => formatter.debug_tuple("Text").field(value).finish(),
            Self::Secret { .. } => formatter.write_str("Secret(<redacted>)"),
            Self::Address { value } => formatter.debug_tuple("Address").field(value).finish(),
            Self::Choice { index } => formatter.debug_tuple("Choice").field(index).finish(),
            Self::Toggle { value } => formatter.debug_tuple("Toggle").field(value).finish(),
        }
    }
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct SdkConnectionParameter {
    pub key: String,
    pub value: SdkConnectionValue,
}

impl From<DeviceConnectionField> for SdkConnectionField {
    fn from(field: DeviceConnectionField) -> Self {
        Self {
            key: field.key,
            label: field.label,
            kind: field.kind.into(),
            required: field.required,
            retention: field.retention.into(),
        }
    }
}
