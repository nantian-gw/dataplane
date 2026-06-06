#[test]
fn session_tokens_round_trip_for_cookie_transport() {
    let manager = session_manager();
    let policy = cookie_policy();
    let selected = selected_backend();
    let mut response = ResponseHeader::build(200, None).expect("response");

    manager
        .write_response_session(&mut response, &policy, &selected, None)
        .expect("cookie should be written");

    let cookie_header = response
        .headers
        .get("set-cookie")
        .and_then(|value| value.to_str().ok())
        .expect("set-cookie header");
    let cookie_value = cookie_header
        .split(';')
        .next()
        .and_then(|value| value.split_once('='))
        .map(|(_, value)| value.to_string())
        .expect("cookie token");

    let mut request = RequestHeader::build("GET", b"/app", None).expect("request");
    request
        .insert_header("cookie", format!("{}={cookie_value}", policy.session_name))
        .expect("cookie header");

    let resolved = manager
        .resolve_request_session(&request, &policy)
        .expect("session should resolve");
    assert_eq!(resolved.target.backend_name, "default/echo:8080");
    assert_eq!(resolved.target.endpoint.address, "10.0.0.10");
}

#[test]
fn tampered_session_tokens_are_rejected() {
    let manager = session_manager();
    let policy = cookie_policy();
    let selected = selected_backend();
    let mut response = ResponseHeader::build(200, None).expect("response");

    manager
        .write_response_session(&mut response, &policy, &selected, None)
        .expect("cookie should be written");
    let cookie_header = response
        .headers
        .get("set-cookie")
        .and_then(|value| value.to_str().ok())
        .expect("set-cookie header");
    let cookie_pair = cookie_header
        .split(';')
        .next()
        .expect("cookie pair")
        .to_string();
    let tampered = format!("{cookie_pair}tampered");

    let mut request = RequestHeader::build("GET", b"/app", None).expect("request");
    request
        .insert_header("cookie", tampered)
        .expect("cookie header");

    assert!(manager.resolve_request_session(&request, &policy).is_none());
}

#[test]
fn cookie_path_uses_matched_http_path() {
    assert_eq!(cookie_path(&selected_backend()), "/app");
    assert_eq!(
        longest_literal_cookie_path("/v1/users/.*/detail"),
        "/v1/users"
    );
    assert_eq!(longest_literal_cookie_path(".*"), "/");
}
