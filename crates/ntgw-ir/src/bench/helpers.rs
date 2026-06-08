use crate::BackendEndpoint;
use ntgw_proto::gateway::control::v1 as proto;

pub(super) fn bench_backend_port(
    base_port: u32,
    listener_index: usize,
    route_index: usize,
    backend_index: usize,
    backends_per_route: usize,
    routes_per_listener: usize,
) -> u32 {
    let offset = listener_index * routes_per_listener * backends_per_route
        + route_index * backends_per_route
        + backend_index;
    base_port + (offset % 10_000) as u32
}

#[allow(clippy::too_many_arguments)]
pub(super) fn bench_endpoints(
    listener_index: usize,
    route_index: usize,
    backend_index: usize,
    routes_per_listener: usize,
    backends_per_route: usize,
    endpoints_per_backend: usize,
    port: u32,
    suffix_seed: usize,
) -> Vec<BackendEndpoint> {
    (0..endpoints_per_backend)
        .map(|endpoint_index| BackendEndpoint {
            address: bench_ipv4(
                listener_index * routes_per_listener * backends_per_route
                    + route_index * backends_per_route
                    + backend_index,
                endpoint_index + suffix_seed,
            ),
            port,
            healthy: true,
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
pub(super) fn bench_proto_endpoints(
    listener_index: usize,
    route_index: usize,
    backend_index: usize,
    routes_per_listener: usize,
    backends_per_route: usize,
    endpoints_per_backend: usize,
    port: u32,
    suffix_seed: usize,
) -> Vec<proto::BackendEndpoint> {
    bench_endpoints(
        listener_index,
        route_index,
        backend_index,
        routes_per_listener,
        backends_per_route,
        endpoints_per_backend,
        port,
        suffix_seed,
    )
    .into_iter()
    .map(|endpoint| proto::BackendEndpoint {
        address: endpoint.address,
        port: endpoint.port,
        healthy: endpoint.healthy,
        zone: String::new(),
    })
    .collect()
}

pub(super) fn bench_ipv4(seed: usize, suffix: usize) -> String {
    let a = ((seed / 65_025) % 250) + 1;
    let b = ((seed / 255) % 250) + 1;
    let c = (seed % 250) + 1;
    let d = (suffix % 250) + 1;
    format!("10.{a}.{b}.{}", ((c + d - 1) % 250) + 1)
}
