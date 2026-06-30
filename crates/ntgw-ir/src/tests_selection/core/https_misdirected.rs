#[test]
fn https_misdirected_request_matches_sni_and_host_best_listeners() {
    let mut snapshot = Snapshot {
        listeners: vec![
            https_listener("default/gw/https", 443, vec![]),
            https_listener(
                "default/gw/https-with-hostname",
                443,
                vec!["second-example.org"],
            ),
            https_listener(
                "default/gw/https-with-wildcard-hostname",
                443,
                vec!["*.wildcard.org"],
            ),
            https_listener(
                "default/gw/https-with-hostname-matching-wildcard",
                443,
                vec!["fourth-example.wildcard.org"],
            ),
        ],
        ..Snapshot::default()
    };
    snapshot.rebuild_runtime_indexes();

    let cases = [
        ("example.org", "example.org", false),
        ("example.org", "second-example.org", true),
        ("example.org", "unknown-example.org", false),
        ("second-example.org", "second-example.org", false),
        ("second-example.org", "example.org", true),
        ("second-example.org", "unknown-example.org", true),
        (
            "third-example.wildcard.org",
            "third-example.wildcard.org",
            false,
        ),
        (
            "third-example.wildcard.org",
            "fith-example.wildcard.org",
            false,
        ),
        (
            "third-example.wildcard.org",
            "fourth-example.wildcard.org",
            true,
        ),
        ("third-example.wildcard.org", "second-example.org", true),
        ("third-example.wildcard.org", "unknown-example.org", true),
        (
            "fourth-example.wildcard.org",
            "fourth-example.wildcard.org",
            false,
        ),
        (
            "fourth-example.wildcard.org",
            "fith-example.wildcard.org",
            true,
        ),
        ("unknown-example.org", "example.org", false),
        ("unknown-example.org", "unknown-example.org", false),
    ];

    for (sni, host, want) in cases {
        let request = RequestMeta::with_port(
            Some(host.to_string()),
            443,
            "/detect-misdirected-requests",
            "GET",
            BTreeMap::new(),
        );
        assert_eq!(
            snapshot.https_request_is_misdirected(&request, Some(sni)),
            want,
            "sni={sni} host={host}"
        );
    }
}

#[test]
fn https_misdirected_request_ignores_plain_http_and_missing_sni() {
    let mut snapshot = Snapshot {
        listeners: vec![
            Listener {
                name: "default/gw/http".to_string().into(),
                port: 80,
                protocol: "LISTENER_PROTOCOL_HTTP".to_string().into(),
                hostnames: vec!["example.org".to_string()],
                ..Listener::default()
            },
            https_listener(
                "default/gw/https-with-hostname",
                443,
                vec!["second-example.org"],
            ),
        ],
        ..Snapshot::default()
    };
    snapshot.rebuild_runtime_indexes();

    let http_request =
        RequestMeta::with_port(Some("other.example".to_string()), 80, "/", "GET", BTreeMap::new());
    assert!(!snapshot.https_request_is_misdirected(&http_request, Some("example.org")));

    let https_request = RequestMeta::with_port(
        Some("other.example".to_string()),
        443,
        "/",
        "GET",
        BTreeMap::new(),
    );
    assert!(!snapshot.https_request_is_misdirected(&https_request, None));
}

#[test]
fn https_misdirected_request_scores_only_https_listeners() {
    let mut snapshot = Snapshot {
        listeners: vec![
            https_listener("default/gw/https", 443, vec![]),
            https_listener(
                "default/gw/https-with-wildcard-hostname",
                443,
                vec!["*.example.org"],
            ),
            Listener {
                name: "default/gw/plain-http-same-port".to_string().into(),
                port: 443,
                protocol: "LISTENER_PROTOCOL_HTTP".to_string().into(),
                hostnames: vec!["api.example.org".to_string()],
                ..Listener::default()
            },
        ],
        ..Snapshot::default()
    };
    snapshot.rebuild_runtime_indexes();

    let request = RequestMeta::with_port(
        Some("api.example.org".to_string()),
        443,
        "/",
        "GET",
        BTreeMap::new(),
    );
    assert!(snapshot.https_request_is_misdirected(&request, Some("example.org")));
}

fn https_listener(name: &str, port: u32, hostnames: Vec<&str>) -> Listener {
    Listener {
        name: name.to_string().into(),
        port,
        protocol: "LISTENER_PROTOCOL_HTTPS".to_string().into(),
        hostnames: hostnames.into_iter().map(str::to_string).collect(),
        ..Listener::default()
    }
}
