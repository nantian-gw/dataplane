use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var_os("PROTOC").is_none() {
        unsafe { std::env::set_var("PROTOC", protoc_bin_vendored::protoc_bin_path()?) };
    }

    let local_proto_root = PathBuf::from("proto");
    let proto_include = std::env::var_os("PROTOC_INCLUDE")
        .map(PathBuf::from)
        .unwrap_or(protoc_bin_vendored::include_path()?);
    let ext_auth_proto = local_proto_root.join("envoy/service/auth/v3/external_auth.proto");
    let attribute_context_proto =
        local_proto_root.join("envoy/service/auth/v3/attribute_context.proto");
    let core_proto = local_proto_root.join("envoy/config/core/v3/base.proto");
    let http_status_proto = local_proto_root.join("envoy/type/v3/http_status.proto");
    let status_proto = local_proto_root.join("google/rpc/status.proto");

    tonic_prost_build::configure()
        .build_server(true)
        .compile_protos(
            &[
                ext_auth_proto,
                attribute_context_proto,
                core_proto,
                http_status_proto,
                status_proto,
            ],
            &[local_proto_root, proto_include],
        )?;

    println!("cargo:rerun-if-changed=proto/envoy/config/core/v3/base.proto");
    println!("cargo:rerun-if-changed=proto/envoy/service/auth/v3/attribute_context.proto");
    println!("cargo:rerun-if-changed=proto/envoy/service/auth/v3/external_auth.proto");
    println!("cargo:rerun-if-changed=proto/envoy/type/v3/http_status.proto");
    println!("cargo:rerun-if-changed=proto/google/rpc/status.proto");
    println!("cargo:rerun-if-env-changed=PROTOC");
    println!("cargo:rerun-if-env-changed=PROTOC_INCLUDE");
    Ok(())
}
