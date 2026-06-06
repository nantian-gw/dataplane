#[test]
fn reports_ephemeral_secret_usage_when_secret_is_generated() {
    let options = SessionPersistenceOptions::build(None, None).expect("options");
    assert!(options.uses_ephemeral_secret());

    let configured =
        SessionPersistenceOptions::build(Some(b"0123456789abcdef0123456789abcdef".to_vec()), None)
            .expect("configured options");
    assert!(!configured.uses_ephemeral_secret());
}

#[test]
fn deterministic_signing_different_managers_same_key_produce_verifiable_tokens() {
    let key = sha2::Sha256::digest(b"prod-shared-secret").to_vec();
    let manager_a = SessionManager::new(
        SessionPersistenceOptions::build(Some(key.clone()), None).expect("manager a"),
    );
    let manager_b = SessionManager::new(
        SessionPersistenceOptions::build(Some(key), None).expect("manager b"),
    );
    let policy = cookie_policy();
    let selected = selected_backend();

    let mut response = ResponseHeader::build(200, None).expect("response");
    manager_a
        .write_response_session(&mut response, &policy, &selected, None)
        .expect("token should be written");

    let token = response
        .headers
        .get("set-cookie")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(';').next())
        .and_then(|v| v.split_once('='))
        .map(|(_, val)| val.to_string())
        .expect("cookie token");

    let mut request = RequestHeader::build("GET", b"/app", None).expect("request");
    request
        .insert_header("cookie", format!("{}={token}", policy.session_name))
        .expect("cookie header");

    let resolved = manager_b
        .resolve_request_session(&request, &policy)
        .expect("token from manager_a should be verified by manager_b");
    assert_eq!(resolved.target.backend_name, selected.backend_name);
}

#[test]
fn random_key_produces_ephemeral_manager() {
    let manager = SessionManager::new(
        SessionPersistenceOptions::build(None, None).expect("random key"),
    );
    let policy = cookie_policy();
    let selected = selected_backend();

    let mut response = ResponseHeader::build(200, None).expect("response");
    manager
        .write_response_session(&mut response, &policy, &selected, None)
        .expect("token should be written");

    let token = response
        .headers
        .get("set-cookie")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(';').next())
        .and_then(|v| v.split_once('='))
        .map(|(_, val)| val.to_string())
        .expect("cookie token");

    let mut request = RequestHeader::build("GET", b"/app", None).expect("request");
    request
        .insert_header("cookie", format!("{}={token}", policy.session_name))
        .expect("cookie header");

    let resolved = manager
        .resolve_request_session(&request, &policy)
        .expect("token from random manager should self-verify");
    assert_eq!(resolved.target.backend_name, selected.backend_name);
}
