use super::*;

#[test]
fn runtime_options_normalize_tcp_proxy_buffer_bytes() {
    assert_eq!(
        RuntimeOptions {
            tcp_proxy_buffer_bytes: 0,
            ..RuntimeOptions::default()
        }
        .effective_tcp_proxy_buffer_bytes(),
        16 * 1024
    );
    assert_eq!(
        RuntimeOptions {
            tcp_proxy_buffer_bytes: 1_024,
            ..RuntimeOptions::default()
        }
        .effective_tcp_proxy_buffer_bytes(),
        4 * 1024
    );
    assert_eq!(
        RuntimeOptions {
            tcp_proxy_buffer_bytes: 512 * 1024,
            ..RuntimeOptions::default()
        }
        .effective_tcp_proxy_buffer_bytes(),
        256 * 1024
    );
}
