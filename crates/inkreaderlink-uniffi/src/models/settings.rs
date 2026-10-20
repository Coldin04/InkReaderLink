use inkreaderlink_core::{
    SettingChange, SettingDescriptor, SettingKind, SettingValue, SettingsSnapshot,
};

#[derive(Clone, Debug, uniffi::Enum)]
pub enum SdkSettingKind {
    Toggle,
    Choice { options: Vec<String> },
    Number { min: i64, max: i64, step: i64 },
    Text,
}

impl From<SettingKind> for SdkSettingKind {
    fn from(kind: SettingKind) -> Self {
        match kind {
            SettingKind::Toggle => Self::Toggle,
            SettingKind::Choice { options } => Self::Choice { options },
            SettingKind::Number { min, max, step } => Self::Number { min, max, step },
            SettingKind::Text => Self::Text,
        }
    }
}

impl From<SdkSettingKind> for SettingKind {
    fn from(kind: SdkSettingKind) -> Self {
        match kind {
            SdkSettingKind::Toggle => Self::Toggle,
            SdkSettingKind::Choice { options } => Self::Choice { options },
            SdkSettingKind::Number { min, max, step } => Self::Number { min, max, step },
            SdkSettingKind::Text => Self::Text,
        }
    }
}

#[derive(Clone, Debug, uniffi::Enum)]
pub enum SdkSettingValue {
    Toggle { value: bool },
    Choice { index: u32 },
    Number { value: i64 },
    Text { value: String },
}

impl From<SettingValue> for SdkSettingValue {
    fn from(value: SettingValue) -> Self {
        match value {
            SettingValue::Toggle(value) => Self::Toggle { value },
            SettingValue::Choice(index) => Self::Choice { index },
            SettingValue::Number(value) => Self::Number { value },
            SettingValue::Text(value) => Self::Text { value },
        }
    }
}

impl From<SdkSettingValue> for SettingValue {
    fn from(value: SdkSettingValue) -> Self {
        match value {
            SdkSettingValue::Toggle { value } => Self::Toggle(value),
            SdkSettingValue::Choice { index } => Self::Choice(index),
            SdkSettingValue::Number { value } => Self::Number(value),
            SdkSettingValue::Text { value } => Self::Text(value),
        }
    }
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct SdkSettingDescriptor {
    pub key: String,
    pub name: String,
    pub category: String,
    pub kind: SdkSettingKind,
    pub value: SdkSettingValue,
}

impl From<SettingDescriptor> for SdkSettingDescriptor {
    fn from(setting: SettingDescriptor) -> Self {
        Self {
            key: setting.key,
            name: setting.name,
            category: setting.category,
            kind: setting.kind.into(),
            value: setting.value.into(),
        }
    }
}

impl From<SdkSettingDescriptor> for SettingDescriptor {
    fn from(setting: SdkSettingDescriptor) -> Self {
        Self {
            key: setting.key,
            name: setting.name,
            category: setting.category,
            kind: setting.kind.into(),
            value: setting.value.into(),
        }
    }
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct SdkSettingsSnapshot {
    pub settings: Vec<SdkSettingDescriptor>,
}

impl From<SettingsSnapshot> for SdkSettingsSnapshot {
    fn from(snapshot: SettingsSnapshot) -> Self {
        Self {
            settings: snapshot.settings.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<SdkSettingsSnapshot> for SettingsSnapshot {
    fn from(snapshot: SdkSettingsSnapshot) -> Self {
        Self {
            settings: snapshot.settings.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Clone, Debug, uniffi::Record)]
pub struct SdkSettingChange {
    pub key: String,
    pub value: SdkSettingValue,
}

impl From<SdkSettingChange> for SettingChange {
    fn from(change: SdkSettingChange) -> Self {
        Self {
            key: change.key,
            value: change.value.into(),
        }
    }
}
