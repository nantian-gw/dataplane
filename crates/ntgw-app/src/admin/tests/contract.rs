use super::*;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct SurfaceContractDocument {
    #[serde(rename = "schemaVersion")]
    schema_version: u32,
    surfaces: Vec<AdminSurfaceDoc>,
}

#[derive(Debug, Deserialize)]
struct AdminSurfaceDoc {
    name: String,
    endpoints: Vec<AdminRouteContract>,
}

#[test]
fn dataplane_route_contract_matches_machine_readable_surface_doc() {
    let manifest_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/admin-api-surface.json");
    let raw = fs::read_to_string(&manifest_path).expect("read admin API surface contract");
    let document: SurfaceContractDocument =
        serde_json::from_str(&raw).expect("decode admin API surface contract");
    let surface = document
        .surfaces
        .into_iter()
        .find(|surface| surface.name == "dataplane-admin")
        .expect("dataplane-admin surface should be documented");

    assert_eq!(document.schema_version, 1);
    assert_eq!(
        canonicalize_endpoint_docs(surface.endpoints),
        canonicalize_endpoint_docs(documented_route_contracts()),
    );
}

fn canonicalize_endpoint_docs(mut endpoints: Vec<AdminRouteContract>) -> Vec<AdminRouteContract> {
    endpoints.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then(left.method.cmp(&right.method))
            .then(left.auth.cmp(&right.auth))
            .then(left.content_type.cmp(&right.content_type))
    });
    endpoints
}
