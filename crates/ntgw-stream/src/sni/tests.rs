use proptest::{collection::vec, prelude::*, string::string_regex};

use crate::sni::{extract_server_name, tls_record_len};

#[test]
fn extracts_sni_from_client_hello() {
    let hello = build_client_hello("example.com");
    assert_eq!(extract_server_name(&hello).as_deref(), Some("example.com"));
}

#[test]
fn reports_tls_record_length() {
    let hello = build_client_hello("example.com");
    assert_eq!(tls_record_len(&hello), Some(hello.len()));
    assert_eq!(tls_record_len(&hello[..4]), None);
}

#[test]
fn returns_none_for_non_tls_payloads() {
    assert_eq!(extract_server_name(b"plain-text"), None);
    assert_eq!(tls_record_len(b"plain-text"), None);
}

#[test]
fn returns_none_for_non_client_hello_records() {
    let mut hello = build_client_hello("example.com");
    hello[5] = 0x02;
    assert_eq!(extract_server_name(&hello), None);
}

#[test]
fn returns_none_for_truncated_server_name_extension() {
    let mut hello = build_client_hello("example.com");
    hello.truncate(hello.len() - 3);
    assert_eq!(extract_server_name(&hello), None);
}

proptest! {
    #[test]
    fn property_extracts_generated_sni_from_client_hello(host in hostname_strategy()) {
        let hello = build_client_hello(host.as_str());
        let extracted = extract_server_name(&hello);

        prop_assert_eq!(tls_record_len(&hello), Some(hello.len()));
        prop_assert_eq!(extracted.as_deref(), Some(host.as_str()));
    }
}

proptest! {
    #[test]
    fn property_truncated_client_hello_never_yields_sni(host in hostname_strategy()) {
        let hello = build_client_hello(host.as_str());

        for len in 0..hello.len() {
            prop_assert_eq!(extract_server_name(&hello[..len]), None);
        }
    }
}

fn build_client_hello(host: &str) -> Vec<u8> {
    let mut body = Vec::new();
    body.extend_from_slice(&[0x03, 0x03]);
    body.extend_from_slice(&[0; 32]);
    body.push(0);
    body.extend_from_slice(&[0x00, 0x02, 0x00, 0x2f]);
    body.extend_from_slice(&[0x01, 0x00]);

    let mut server_name = Vec::new();
    server_name.extend_from_slice(&(host.len() as u16 + 3).to_be_bytes());
    server_name.push(0);
    server_name.extend_from_slice(&(host.len() as u16).to_be_bytes());
    server_name.extend_from_slice(host.as_bytes());

    let mut extensions = Vec::new();
    extensions.extend_from_slice(&[0x00, 0x00]);
    extensions.extend_from_slice(&(server_name.len() as u16).to_be_bytes());
    extensions.extend_from_slice(&server_name);

    body.extend_from_slice(&(extensions.len() as u16).to_be_bytes());
    body.extend_from_slice(&extensions);

    let mut handshake = vec![
        0x01,
        ((body.len() >> 16) & 0xff) as u8,
        ((body.len() >> 8) & 0xff) as u8,
        (body.len() & 0xff) as u8,
    ];
    handshake.extend_from_slice(&body);

    let mut record = Vec::new();
    record.extend_from_slice(&[0x16, 0x03, 0x01]);
    record.extend_from_slice(&(handshake.len() as u16).to_be_bytes());
    record.extend_from_slice(&handshake);
    record
}

fn hostname_strategy() -> BoxedStrategy<String> {
    vec(
        string_regex("[a-z][a-z0-9]{0,8}").expect("hostname label regex"),
        2..4,
    )
    .prop_map(|labels| labels.join("."))
    .boxed()
}
