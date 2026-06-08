use std::collections::{BTreeMap, HashMap};

use ntgw_ir::{
    DirectResponseFilter, ExtensionFilter, Filter, HttpMatch, HttpRoute, HttpRule, RequestMeta,
    Snapshot,
};
use ntgw_proto::gateway::control::v1 as proto;
use prost_types::{Struct, Value, value::Kind};

#[path = "extension_filters/external_auth.rs"]
mod external_auth;
#[path = "extension_filters/proto_flattened.rs"]
mod proto_flattened;
#[path = "extension_filters/proto_nested.rs"]
mod proto_nested;
#[path = "extension_filters/selection.rs"]
mod selection;

fn string_value(value: &str) -> Value {
    Value {
        kind: Some(Kind::StringValue(value.to_string())),
    }
}

fn number_value(value: f64) -> Value {
    Value {
        kind: Some(Kind::NumberValue(value)),
    }
}

fn bool_value(value: bool) -> Value {
    Value {
        kind: Some(Kind::BoolValue(value)),
    }
}

fn struct_value(value: Struct) -> Value {
    Value {
        kind: Some(Kind::StructValue(value)),
    }
}
