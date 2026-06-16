use std::collections::BTreeSet;

use ntgw_proto::gateway::control::v1::ConfigSnapshot;

pub(crate) const FEATURE_CORE_V1: &str = "core.v1";
pub(crate) const FEATURE_ROUTE_LABELS_V1: &str = "route.labels.v1";
pub(crate) const FEATURE_BACKEND_AI_SERVICE_V1: &str = "backend.ai_service.v1";
pub(crate) const FEATURE_BACKEND_TOKEN_POLICY_V1: &str = "backend.token_policy.v1";
pub(crate) const FEATURE_BACKEND_WASM_PLUGIN_V1: &str = "backend.wasm_plugin.v1";

pub(crate) fn supported_features() -> Vec<String> {
    canonicalize_supported_features([
        FEATURE_CORE_V1,
        FEATURE_ROUTE_LABELS_V1,
        FEATURE_BACKEND_AI_SERVICE_V1,
        FEATURE_BACKEND_TOKEN_POLICY_V1,
        FEATURE_BACKEND_WASM_PLUGIN_V1,
    ])
}

pub(crate) fn canonicalize_supported_features(
    items: impl IntoIterator<Item = impl AsRef<str>>,
) -> Vec<String> {
    items
        .into_iter()
        .filter_map(|item| {
            let trimmed = item.as_ref().trim();
            (!trimmed.is_empty()).then(|| trimmed.to_string())
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub(crate) fn preflight_required_features(
    snapshot: &ConfigSnapshot,
    supported: &[String],
) -> Result<(), String> {
    let supported = canonicalize_supported_features(supported.iter().map(String::as_str));
    let missing =
        canonicalize_supported_features(snapshot.required_features.iter().map(String::as_str))
            .into_iter()
            .filter(|feature| supported.binary_search(feature).is_err())
            .collect::<Vec<_>>();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "snapshot requires unsupported features: {}",
            missing.join(", ")
        ))
    }
}
