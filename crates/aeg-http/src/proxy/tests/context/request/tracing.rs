#[test]
fn disabled_request_span_does_not_create_or_inject_traceparent() {
    let headers = BTreeMap::new();
    let mut ctx = RequestContext {
        method: "GET".to_string(),
        path: "/orders".to_string(),
        host: "api.example.com".to_string(),
        ..RequestContext::default()
    };
    let mut upstream = RequestHeader::build("GET", b"/orders", None).expect("request");

    start_request_span_if_enabled(&mut ctx, &headers, false);
    inject_request_span_context(&ctx, &mut upstream);

    assert!(ctx.request_span.is_none());
    assert!(upstream.headers.get("traceparent").is_none());
}

#[test]
fn request_span_injects_new_traceparent_without_parent_context() {
    let provider = SdkTracerProvider::builder().build();
    let tracer = provider.tracer("aeg-http-test");
    let subscriber =
        tracing_subscriber::registry().with(tracing_opentelemetry::layer().with_tracer(tracer));

    tracing::subscriber::with_default(subscriber, || {
        let headers = BTreeMap::new();
        let mut ctx = RequestContext {
            method: "GET".to_string(),
            path: "/orders".to_string(),
            host: "api.example.com".to_string(),
            ..RequestContext::default()
        };
        let mut upstream = RequestHeader::build("GET", b"/orders", None).expect("request");

        start_request_span(&mut ctx, &headers);
        inject_request_span_context(&ctx, &mut upstream);

        let traceparent = upstream
            .headers
            .get("traceparent")
            .and_then(|value| value.to_str().ok())
            .expect("traceparent header");

        assert_valid_traceparent(traceparent);
    });
}

#[test]
fn request_span_continues_inbound_traceparent_trace_id() {
    let provider = SdkTracerProvider::builder().build();
    let tracer = provider.tracer("aeg-http-test");
    let subscriber =
        tracing_subscriber::registry().with(tracing_opentelemetry::layer().with_tracer(tracer));
    let inbound_traceparent = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";

    tracing::subscriber::with_default(subscriber, || {
        let headers = BTreeMap::from([(
            "traceparent".to_string(),
            vec![inbound_traceparent.to_string()],
        )]);
        let mut ctx = RequestContext {
            method: "POST".to_string(),
            path: "/checkout".to_string(),
            host: "shop.example.com".to_string(),
            ..RequestContext::default()
        };
        let mut upstream = RequestHeader::build("POST", b"/checkout", None).expect("request");

        start_request_span(&mut ctx, &headers);
        inject_request_span_context(&ctx, &mut upstream);

        let propagated = upstream
            .headers
            .get("traceparent")
            .and_then(|value| value.to_str().ok())
            .expect("traceparent header");

        assert_valid_traceparent(propagated);
        assert_eq!(
            traceparent_trace_id(propagated),
            traceparent_trace_id(inbound_traceparent)
        );
        assert_ne!(propagated, inbound_traceparent);
    });
}
