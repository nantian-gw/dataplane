proptest! {
    #[test]
    fn session_tokens_round_trip_for_generated_backends_and_transports(
        session_name in session_name_strategy(),
        backend_name in backend_name_strategy(),
        address_tail in 1u8..=250,
        port in 1u32..65535,
        use_header_transport in any::<bool>(),
    ) {
        let manager = session_manager();
        let policy = SessionPersistence {
            session_name: session_name.clone(),
            session_type: if use_header_transport {
                "Header".to_string()
            } else {
                "Cookie".to_string()
            },
            absolute_timeout: Some(Duration::from_secs(300)),
            idle_timeout: Some(Duration::from_secs(60)),
            cookie: if use_header_transport {
                None
            } else {
                Some(aeg_ir::CookieConfig {
                    lifetime_type: "Permanent".to_string(),
                })
            },
        };
        let selected = SelectedBackend {
            backend_name: backend_name.clone(),
            backend: BackendEndpoint {
                address: format!("10.0.0.{address_tail}"),
                port,
                healthy: true,
            },
            ..selected_backend()
        };
        let mut response = ResponseHeader::build(200, None).expect("response");

        manager
            .write_response_session(&mut response, &policy, &selected, None)
            .expect("session token should be written");

        let mut request = RequestHeader::build("GET", b"/app", None).expect("request");
        if use_header_transport {
            let token = response
                .headers
                .get(policy.session_name.as_str())
                .and_then(|value| value.to_str().ok())
                .expect("session header token")
                .to_string();
            request
                .insert_header(policy.session_name.clone(), token)
                .expect("session header");
        } else {
            let token = response
                .headers
                .get("set-cookie")
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.split(';').next())
                .and_then(|value| value.split_once('='))
                .map(|(_, value)| value.to_string())
                .expect("set-cookie token");
            request
                .insert_header("cookie", format!("{}={token}", policy.session_name))
                .expect("cookie header");
        }

        let resolved = manager
            .resolve_request_session(&request, &policy)
            .expect("session should resolve");
        prop_assert_eq!(resolved.target.backend_name, backend_name);
        prop_assert_eq!(resolved.target.endpoint.address, format!("10.0.0.{address_tail}"));
        prop_assert_eq!(resolved.target.endpoint.port, port);
    }
}
