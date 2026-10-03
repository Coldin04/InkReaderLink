//! Device-independent editable setting model and validation.

use std::collections::{HashMap, HashSet};

use serde_json::{Map, Value};

use crate::{
    SdkError, SettingChange, SettingDescriptor, SettingKind, SettingValue, SettingsSnapshot,
};

const MAX_SETTINGS: usize = 256;
const MAX_OPTIONS: usize = 256;
const MAX_LABEL_BYTES: usize = 256;
const MAX_TEXT_BYTES: usize = 1024;

/// Parses the firmware's dynamic setting descriptors, rejecting malformed metadata.
///
/// Identical descriptors with the same key are coalesced so clients can tolerate
/// firmware that exposes one setting in multiple display categories. Conflicting
/// duplicate keys remain malformed because their update semantics are ambiguous.
///
/// # Errors
/// Returns `RemoteFailure` if any setting cannot be safely represented.
pub fn parse_settings(body: &[u8]) -> Result<SettingsSnapshot, SdkError> {
    let items: Vec<Value> = serde_json::from_slice(body)
        .map_err(|error| SdkError::RemoteFailure(format!("invalid settings list: {error}")))?;
    if items.len() > MAX_SETTINGS {
        return Err(SdkError::RemoteFailure("too many settings".to_owned()));
    }
    let mut indexes_by_key: HashMap<String, usize> = HashMap::with_capacity(items.len());
    let mut settings: Vec<SettingDescriptor> = Vec::with_capacity(items.len());
    for item in items {
        let setting = parse_descriptor(&item)?;
        if let Some(index) = indexes_by_key.get(&setting.key) {
            let existing = &settings[*index];
            if existing.name == setting.name
                && existing.kind == setting.kind
                && existing.value == setting.value
            {
                continue;
            }
            return Err(malformed(&format!(
                "conflicting duplicate setting key: {}",
                setting.key
            )));
        }
        indexes_by_key.insert(setting.key.clone(), settings.len());
        settings.push(setting);
    }
    Ok(SettingsSnapshot { settings })
}

fn parse_descriptor(item: &Value) -> Result<SettingDescriptor, SdkError> {
    let object = item
        .as_object()
        .ok_or_else(|| malformed("setting is not an object"))?;
    let key = string_field(object, "key")?;
    if key.is_empty()
        || key.len() > 64
        || !key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.'))
    {
        return Err(malformed("invalid setting key"));
    }
    let name = display_field(object, "name")?;
    let category = display_field(object, "category")?;
    let value = object
        .get("value")
        .ok_or_else(|| malformed("missing setting value"))?;
    let (kind, value) = match string_field(object, "type")? {
        "toggle" => {
            let number = value
                .as_i64()
                .ok_or_else(|| malformed("invalid toggle value"))?;
            if !matches!(number, 0 | 1) {
                return Err(malformed("invalid toggle value"));
            }
            (SettingKind::Toggle, SettingValue::Toggle(number == 1))
        }
        "enum" => {
            let raw = object
                .get("options")
                .and_then(Value::as_array)
                .ok_or_else(|| malformed("invalid setting options"))?;
            if raw.is_empty() || raw.len() > MAX_OPTIONS {
                return Err(malformed("invalid setting options"));
            }
            let options = raw
                .iter()
                .map(|option| {
                    let text = option
                        .as_str()
                        .ok_or_else(|| malformed("invalid setting option"))?;
                    validate_display(text)?;
                    Ok(text.to_owned())
                })
                .collect::<Result<Vec<_>, SdkError>>()?;
            let index = value
                .as_u64()
                .ok_or_else(|| malformed("invalid enum index"))?;
            if index >= options.len() as u64 {
                return Err(malformed("invalid enum index"));
            }
            (
                SettingKind::Choice { options },
                SettingValue::Choice(
                    u32::try_from(index).map_err(|_| malformed("invalid enum index"))?,
                ),
            )
        }
        "value" => {
            let min = integer_field(object, "min")?;
            let max = integer_field(object, "max")?;
            let step = integer_field(object, "step")?;
            if min > max || step <= 0 {
                return Err(malformed("invalid numeric bounds"));
            }
            let number = value
                .as_i64()
                .ok_or_else(|| malformed("invalid numeric value"))?;
            if number < min || number > max {
                return Err(malformed("numeric value out of bounds"));
            }
            (
                SettingKind::Number { min, max, step },
                SettingValue::Number(number),
            )
        }
        "string" => {
            let text = value
                .as_str()
                .ok_or_else(|| malformed("invalid text value"))?;
            if text.len() > MAX_TEXT_BYTES {
                return Err(malformed("text value too long"));
            }
            (SettingKind::Text, SettingValue::Text(text.to_owned()))
        }
        _ => return Err(malformed("unsupported setting type")),
    };
    Ok(SettingDescriptor {
        key: key.to_owned(),
        name,
        category,
        kind,
        value,
    })
}

/// Validates a partial update against a fresh setting snapshot and builds firmware JSON.
///
/// # Errors
/// Returns `InvalidArgument` for unknown keys, duplicate keys, or values outside the declared shape.
pub fn encode_changes(
    snapshot: &SettingsSnapshot,
    changes: &[SettingChange],
) -> Result<Vec<u8>, SdkError> {
    if changes.is_empty() || changes.len() > MAX_SETTINGS {
        return Err(SdkError::InvalidArgument(
            "settings update must contain 1–256 changes".to_owned(),
        ));
    }
    let mut seen = HashSet::new();
    let mut update = Map::new();
    for change in changes {
        if !seen.insert(&change.key) {
            return Err(SdkError::InvalidArgument(format!(
                "duplicate setting key: {}",
                change.key
            )));
        }
        let setting = snapshot
            .settings
            .iter()
            .find(|item| item.key == change.key)
            .ok_or_else(|| {
                SdkError::InvalidArgument(format!("unknown setting key: {}", change.key))
            })?;
        let value = match (&setting.kind, &change.value) {
            (SettingKind::Toggle, SettingValue::Toggle(value)) => Value::from(i64::from(*value)),
            (SettingKind::Choice { options }, SettingValue::Choice(index))
                if (*index as usize) < options.len() =>
            {
                Value::from(*index)
            }
            (SettingKind::Number { min, max, step }, SettingValue::Number(value))
                if value >= min
                    && value <= max
                    && (i128::from(*value) - i128::from(*min)).rem_euclid(i128::from(*step))
                        == 0 =>
            {
                Value::from(*value)
            }
            (SettingKind::Text, SettingValue::Text(value)) if value.len() <= MAX_TEXT_BYTES => {
                Value::from(value.clone())
            }
            _ => {
                return Err(SdkError::InvalidArgument(format!(
                    "invalid value for setting: {}",
                    change.key
                )));
            }
        };
        update.insert(change.key.clone(), value);
    }
    serde_json::to_vec(&update)
        .map_err(|error| SdkError::InvalidArgument(format!("invalid settings update: {error}")))
}

fn malformed(message: &str) -> SdkError {
    SdkError::RemoteFailure(message.to_owned())
}

fn string_field<'a>(object: &'a Map<String, Value>, key: &str) -> Result<&'a str, SdkError> {
    object
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| malformed(&format!("missing or invalid {key}")))
}

fn display_field(object: &Map<String, Value>, key: &str) -> Result<String, SdkError> {
    let text = string_field(object, key)?;
    validate_display(text)?;
    Ok(text.to_owned())
}

fn validate_display(text: &str) -> Result<(), SdkError> {
    if text.is_empty() || text.len() > MAX_LABEL_BYTES || text.chars().any(char::is_control) {
        return Err(malformed("invalid setting label"));
    }
    Ok(())
}

fn integer_field(object: &Map<String, Value>, key: &str) -> Result<i64, SdkError> {
    object
        .get(key)
        .and_then(Value::as_i64)
        .ok_or_else(|| malformed(&format!("missing or invalid {key}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    const RESPONSE: &str = r#"[
        {"key":"showHiddenFiles","name":"Show hidden files","category":"Files","type":"toggle","value":1},
        {"key":"fontSize","name":"Font size","category":"Reader","type":"enum","value":1,"options":["12 pt","14 pt"]},
        {"key":"offset","name":"Offset","category":"Reader","type":"value","value":-2,"min":-10,"max":10,"step":2},
        {"key":"label","name":"Label","category":"Reader","type":"string","value":"hello"}
    ]"#;

    #[test]
    fn parses_all_types_and_encodes_only_changed_keys() {
        let snapshot = parse_settings(RESPONSE.as_bytes()).unwrap();
        assert_eq!(snapshot.settings.len(), 4);
        assert_eq!(snapshot.settings[1].value, SettingValue::Choice(1));
        let body = encode_changes(
            &snapshot,
            &[
                SettingChange {
                    key: "showHiddenFiles".to_owned(),
                    value: SettingValue::Toggle(false),
                },
                SettingChange {
                    key: "offset".to_owned(),
                    value: SettingValue::Number(-4),
                },
            ],
        )
        .unwrap();
        let value: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value, serde_json::json!({"showHiddenFiles":0,"offset":-4}));
    }

    #[test]
    fn rejects_untrusted_descriptors_and_invalid_changes() {
        assert!(
            parse_settings(
                br#"[{"key":"../bad","name":"bad","category":"x","type":"toggle","value":1}]"#
            )
            .is_err()
        );
        assert!(parse_settings(br#"[{"key":"a","name":"bad","category":"x","type":"enum","value":2,"options":["x"]}]"#).is_err());
        let snapshot = parse_settings(RESPONSE.as_bytes()).unwrap();
        for change in [
            SettingChange {
                key: "unknown".to_owned(),
                value: SettingValue::Toggle(true),
            },
            SettingChange {
                key: "fontSize".to_owned(),
                value: SettingValue::Choice(2),
            },
            SettingChange {
                key: "offset".to_owned(),
                value: SettingValue::Number(-3),
            },
            SettingChange {
                key: "label".to_owned(),
                value: SettingValue::Text("x".repeat(MAX_TEXT_BYTES + 1)),
            },
        ] {
            assert!(matches!(
                encode_changes(&snapshot, &[change]),
                Err(SdkError::InvalidArgument(_))
            ));
        }
    }

    #[test]
    fn coalesces_matching_duplicate_keys_from_multiple_categories() {
        let snapshot = parse_settings(
            br#"[
                {"key":"screenInverted","name":"Night mode","category":"Display","type":"toggle","value":0},
                {"key":"screenInverted","name":"Night mode","category":"Reader","type":"toggle","value":0}
            ]"#,
        )
        .unwrap();

        assert_eq!(snapshot.settings.len(), 1);
        assert_eq!(snapshot.settings[0].category, "Display");
    }

    #[test]
    fn rejects_conflicting_duplicate_keys() {
        let error = parse_settings(
            br#"[
                {"key":"screenInverted","name":"Night mode","category":"Display","type":"toggle","value":0},
                {"key":"screenInverted","name":"Night mode","category":"Reader","type":"toggle","value":1}
            ]"#,
        )
        .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("conflicting duplicate setting key: screenInverted")
        );
    }
}
