use std::path::PathBuf;
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var_os("PROTOC").is_none() {
        std::env::set_var("PROTOC", protoc_bin_vendored::protoc_bin_path()?);
    }

    let proto_version = std::env::var("NTGW_PROTO_VERSION").unwrap_or_else(|_| "main".to_string());
    let out_dir = PathBuf::from(std::env::var("OUT_DIR")?);
    let proto_dir = out_dir.join("proto");
    let control_proto_path = proto_dir.join("gateway/control/v1/control.proto");

    if !control_proto_path.exists() {
        std::fs::create_dir_all(control_proto_path.parent().unwrap())?;

        let url = format!("https://raw.githubusercontent.com/nantian-gw/proto/{proto_version}/gateway/control/v1/control.proto");

        let agent = ureq::AgentBuilder::new()
            .timeout(Duration::from_secs(60))
            .build();

        let mut last_err = None;
        for attempt in 0..3 {
            match agent.get(&url).call() {
                Ok(response) => {
                    let body = response.into_string()?;
                    std::fs::write(&control_proto_path, &body)?;
                    last_err = None;
                    break;
                }
                Err(e) => {
                    last_err = Some(e);
                    if attempt < 2 {
                        std::thread::sleep(Duration::from_secs(2));
                    }
                }
            }
        }
        if let Some(e) = last_err {
            return Err(Box::new(e));
        }
    }

    let local_proto_root = PathBuf::from("proto");
    let proto_include = std::env::var_os("PROTOC_INCLUDE")
        .map(PathBuf::from)
        .unwrap_or(protoc_bin_vendored::include_path()?);
    let control_proto = control_proto_path;
    let ext_auth_proto = local_proto_root.join("envoy/service/auth/v3/external_auth.proto");
    let attribute_context_proto =
        local_proto_root.join("envoy/service/auth/v3/attribute_context.proto");
    let core_proto = local_proto_root.join("envoy/config/core/v3/base.proto");
    let http_status_proto = local_proto_root.join("envoy/type/v3/http_status.proto");
    let status_proto = local_proto_root.join("google/rpc/status.proto");

    tonic_build::configure()
        .build_server(false)
        .compile_protos(&[control_proto], &[proto_dir, proto_include.clone()])?;

    tonic_build::configure().build_server(true).compile_protos(
        &[
            ext_auth_proto,
            attribute_context_proto,
            core_proto,
            http_status_proto,
            status_proto,
        ],
        &[local_proto_root, proto_include],
    )?;

    println!("cargo:rerun-if-env-changed=NTGW_PROTO_VERSION");
    println!("cargo:rerun-if-changed=proto/envoy/config/core/v3/base.proto");
    println!("cargo:rerun-if-changed=proto/envoy/service/auth/v3/attribute_context.proto");
    println!("cargo:rerun-if-changed=proto/envoy/service/auth/v3/external_auth.proto");
    println!("cargo:rerun-if-changed=proto/envoy/type/v3/http_status.proto");
    println!("cargo:rerun-if-changed=proto/google/rpc/status.proto");
    println!("cargo:rerun-if-env-changed=PROTOC");
    println!("cargo:rerun-if-env-changed=PROTOC_INCLUDE");
    Ok(())
}
