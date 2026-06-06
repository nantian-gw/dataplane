use crate::{BackendPolicy, ConsistentHashPolicy, LoadBalancingPolicy};

include!("tests_load_balancing/proto_decode.rs");
include!("tests_load_balancing/consistent_hash_backends.rs");
include!("tests_load_balancing/consistent_hash_endpoints.rs");
