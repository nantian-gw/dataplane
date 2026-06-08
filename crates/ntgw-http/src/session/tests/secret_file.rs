#[test]
fn session_manager_reloads_secret_key_file() {
    let path = temp_secret_path("session-key");
    fs::write(&path, b"0123456789abcdef0123456789abcdef").expect("write secret");

    let manager = SessionManager::new(SessionPersistenceOptions {
        secret_source: SecretSource::File(
            FileSecretSource::new_with_refresh_interval(PathBuf::from(&path), Duration::ZERO)
                .expect("options"),
        ),
    });
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

    fs::write(&path, b"fedcba9876543210fedcba9876543210").expect("rewrite secret");

    let mut stale_request = RequestHeader::build("GET", b"/app", None).expect("request");
    stale_request
        .insert_header("cookie", format!("{}={cookie_value}", policy.session_name))
        .expect("cookie header");
    assert!(manager
        .resolve_request_session(&stale_request, &policy)
        .is_none());

    let mut rotated_response = ResponseHeader::build(200, None).expect("response");
    manager
        .write_response_session(&mut rotated_response, &policy, &selected, None)
        .expect("cookie should be written");
    let rotated_cookie = rotated_response
        .headers
        .get("set-cookie")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .and_then(|value| value.split_once('='))
        .map(|(_, value)| value.to_string())
        .expect("rotated cookie token");

    let mut rotated_request = RequestHeader::build("GET", b"/app", None).expect("request");
    rotated_request
        .insert_header(
            "cookie",
            format!("{}={rotated_cookie}", policy.session_name),
        )
        .expect("cookie header");
    assert!(manager
        .resolve_request_session(&rotated_request, &policy)
        .is_some());

    let _ = fs::remove_file(path);
}

#[test]
fn session_manager_uses_cached_secret_within_refresh_interval() {
    let path = temp_secret_path("session-key-cache");
    fs::write(&path, b"0123456789abcdef0123456789abcdef").expect("write secret");

    let manager = SessionManager::new(SessionPersistenceOptions {
        secret_source: SecretSource::File(
            FileSecretSource::new_with_refresh_interval(
                PathBuf::from(&path),
                Duration::from_secs(60),
            )
            .expect("options"),
        ),
    });
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

    fs::write(&path, b"fedcba9876543210fedcba9876543210").expect("rewrite secret");

    let mut request = RequestHeader::build("GET", b"/app", None).expect("request");
    request
        .insert_header("cookie", format!("{}={cookie_value}", policy.session_name))
        .expect("cookie header");
    assert!(manager.resolve_request_session(&request, &policy).is_some());

    let _ = fs::remove_file(path);
}
