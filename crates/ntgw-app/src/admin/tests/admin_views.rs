use super::*;

mod circuit_breakers;
mod metrics;
mod overload;
mod rate_limits;
mod runtime_ids;
mod traffic;

async fn authorized_text(app: &axum::Router, uri: &str) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(uri)
                .header(header::AUTHORIZATION, "Bearer top-secret")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("request should succeed");
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body");
    String::from_utf8(body.to_vec()).expect("utf-8")
}

async fn authorized_json(app: &axum::Router, uri: &str) -> serde_json::Value {
    serde_json::from_str(&authorized_text(app, uri).await).expect("json")
}

fn metric_u64(metrics: &str, name: &str) -> u64 {
    metric_raw_value(metrics, name)
        .parse::<u64>()
        .expect("u64 metric")
}

fn metric_f64(metrics: &str, name: &str) -> f64 {
    metric_raw_value(metrics, name)
        .parse::<f64>()
        .expect("f64 metric")
}

fn labeled_metric_u64(metrics: &str, name: &str, label_name: &str, label_value: &str) -> u64 {
    let line = format!("{name}{{{label_name}=\"{label_value}\"}} ");
    metrics
        .lines()
        .find_map(|item| item.strip_prefix(line.as_str()))
        .expect("labeled metric")
        .parse::<u64>()
        .expect("u64 labeled metric")
}

fn metric_raw_value<'a>(metrics: &'a str, name: &str) -> &'a str {
    let prefix = format!("{name} ");
    metrics
        .lines()
        .find_map(|item| item.strip_prefix(prefix.as_str()))
        .expect("metric")
}
