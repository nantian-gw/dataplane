use super::*;

pub(super) fn list_value(value: Option<&Value>) -> Option<&ListValue> {
    match value?.kind.as_ref()? {
        Kind::ListValue(list) => Some(list),
        _ => None,
    }
}

pub(super) fn struct_value(value: &Value) -> Option<&Struct> {
    match value.kind.as_ref()? {
        Kind::StructValue(item) => Some(item),
        _ => None,
    }
}

pub(super) fn string_value(value: &Value) -> Option<String> {
    match value.kind.as_ref()? {
        Kind::StringValue(item) => Some(item.clone()),
        _ => None,
    }
}

pub(super) fn optional_string(value: Option<&Value>) -> Option<String> {
    value.and_then(string_value)
}

pub(super) fn optional_u32(value: Option<&Value>) -> Option<u32> {
    match value?.kind.as_ref()? {
        Kind::NumberValue(item) if *item >= 0.0 => Some(*item as u32),
        _ => None,
    }
}

pub(super) fn optional_bool(value: Option<&Value>) -> Option<bool> {
    match value?.kind.as_ref()? {
        Kind::BoolValue(item) => Some(*item),
        _ => None,
    }
}

pub(super) fn optional_string_map(value: Option<&Value>) -> Option<BTreeMap<String, String>> {
    let item = struct_value(value?)?;
    Some(
        item.fields
            .iter()
            .filter_map(|(key, value)| string_value(value).map(|value| (key.clone(), value)))
            .collect(),
    )
}

pub(super) fn duration_from_proto(value: &ProtoDuration) -> Option<Duration> {
    if value.seconds < 0 || !(0..1_000_000_000).contains(&value.nanos) {
        return None;
    }

    Some(Duration::new(value.seconds as u64, value.nanos as u32))
}
