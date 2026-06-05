use pingora::http::RequestHeader;

use super::super::remove_downstream_close_connection_token;

#[test]
fn connection_header_without_close_is_retained() {
    let mut request = RequestHeader::build("GET", b"/", None).expect("request");
    request
        .insert_header("connection", "keep-alive, upgrade")
        .expect("connection");

    remove_downstream_close_connection_token(&mut request).expect("remove close token");

    assert_eq!(
        request
            .headers
            .get("connection")
            .and_then(|value| value.to_str().ok()),
        Some("keep-alive, upgrade")
    );
}

#[test]
fn connection_header_removes_only_close_token() {
    let mut request = RequestHeader::build("GET", b"/", None).expect("request");
    request
        .insert_header("connection", "keep-alive, close, upgrade")
        .expect("connection");

    remove_downstream_close_connection_token(&mut request).expect("remove close token");

    assert_eq!(
        request
            .headers
            .get("connection")
            .and_then(|value| value.to_str().ok()),
        Some("keep-alive, upgrade")
    );
}
