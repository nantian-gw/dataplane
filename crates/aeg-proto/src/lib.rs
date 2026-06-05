#![forbid(unsafe_code)]

pub mod gateway {
    pub mod control {
        pub mod v1 {
            tonic::include_proto!("gateway.control.v1");
        }
    }
}

pub mod envoy {
    pub mod config {
        pub mod core {
            pub mod v3 {
                tonic::include_proto!("envoy.config.core.v3");
            }
        }
    }

    pub mod service {
        pub mod auth {
            pub mod v3 {
                tonic::include_proto!("envoy.service.auth.v3");
            }
        }
    }

    pub mod r#type {
        pub mod v3 {
            tonic::include_proto!("envoy.r#type.v3");
        }
    }
}

pub mod google {
    pub mod rpc {
        tonic::include_proto!("google.rpc");
    }
}
