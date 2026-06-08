use std::{mem, time::Duration};

use anyhow::{Result, anyhow};
use opentelemetry::{KeyValue, global, trace::TracerProvider};
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{
    Resource,
    propagation::TraceContextPropagator,
    trace::{Sampler, SdkTracerProvider},
};
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::prelude::*;
use tracing_subscriber::{EnvFilter, fmt, fmt::writer::BoxMakeWriter};

static TRACING_WORKER_GUARD: std::sync::OnceLock<WorkerGuard> = std::sync::OnceLock::new();
const DEFAULT_NON_BLOCKING_BUFFERED_LINES: usize = 65_536;

#[derive(Debug, Clone)]
pub struct TracingOptions {
    pub level: String,
    pub format: String,
    pub add_source: bool,
    pub include_target: bool,
    pub include_thread_ids: bool,
    pub include_thread_names: bool,
    pub non_blocking: bool,
    pub non_blocking_buffered_lines: usize,
    pub drop_when_full: bool,
    pub open_telemetry: OpenTelemetryOptions,
}

#[derive(Debug, Clone)]
pub struct OpenTelemetryOptions {
    pub enabled: bool,
    pub endpoint: String,
    pub protocol: String,
    pub timeout_ms: u64,
    pub insecure: bool,
    pub sample_ratio: f64,
    pub service_name: String,
    pub service_namespace: String,
    pub service_instance_id: String,
    pub deployment_environment: String,
}

impl Default for TracingOptions {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
            format: "json".to_string(),
            add_source: false,
            include_target: false,
            include_thread_ids: false,
            include_thread_names: false,
            non_blocking: true,
            non_blocking_buffered_lines: DEFAULT_NON_BLOCKING_BUFFERED_LINES,
            drop_when_full: true,
            open_telemetry: OpenTelemetryOptions::default(),
        }
    }
}

impl Default for OpenTelemetryOptions {
    fn default() -> Self {
        Self {
            enabled: false,
            endpoint: String::new(),
            protocol: "grpc".to_string(),
            timeout_ms: 3_000,
            insecure: false,
            sample_ratio: 1.0,
            service_name: "nantian-dataplane".to_string(),
            service_namespace: String::new(),
            service_instance_id: String::new(),
            deployment_environment: String::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum OpenTelemetryProtocol {
    Grpc,
}

#[derive(Debug, Clone, PartialEq)]
struct ResolvedOpenTelemetryOptions {
    endpoint: String,
    protocol: OpenTelemetryProtocol,
    timeout_ms: u64,
    insecure: bool,
    sample_ratio: f64,
    service_name: String,
    service_namespace: String,
    service_instance_id: String,
    deployment_environment: String,
}

pub fn init_tracing(options: &TracingOptions) -> Result<()> {
    let filter =
        EnvFilter::try_new(options.level.as_str()).unwrap_or_else(|_| EnvFilter::new("info"));
    let resolved_open_telemetry = resolve_open_telemetry_options(&options.open_telemetry)?;
    let (log_writer, log_guard) = build_log_writer(options);

    global::set_text_map_propagator(TraceContextPropagator::new());

    if options.format.eq_ignore_ascii_case("json") {
        if let Some(ref resolved) = resolved_open_telemetry {
            tracing_subscriber::registry()
                .with(filter)
                .with(build_open_telemetry_layer(resolved)?)
                .with(
                    fmt::layer()
                        .json()
                        .with_target(options.include_target)
                        .with_file(options.add_source)
                        .with_line_number(options.add_source)
                        .with_thread_ids(options.include_thread_ids)
                        .with_thread_names(options.include_thread_names)
                        .with_writer(log_writer),
                )
                .init();
        } else {
            tracing_subscriber::registry()
                .with(filter)
                .with(
                    fmt::layer()
                        .json()
                        .with_target(options.include_target)
                        .with_file(options.add_source)
                        .with_line_number(options.add_source)
                        .with_thread_ids(options.include_thread_ids)
                        .with_thread_names(options.include_thread_names)
                        .with_writer(log_writer),
                )
                .init();
        }
    } else if let Some(ref resolved) = resolved_open_telemetry {
        tracing_subscriber::registry()
            .with(filter)
            .with(build_open_telemetry_layer(resolved)?)
            .with(
                fmt::layer()
                    .with_target(options.include_target)
                    .with_file(options.add_source)
                    .with_line_number(options.add_source)
                    .with_thread_ids(options.include_thread_ids)
                    .with_thread_names(options.include_thread_names)
                    .with_writer(log_writer),
            )
            .init();
    } else {
        tracing_subscriber::registry()
            .with(filter)
            .with(
                fmt::layer()
                    .with_target(options.include_target)
                    .with_file(options.add_source)
                    .with_line_number(options.add_source)
                    .with_thread_ids(options.include_thread_ids)
                    .with_thread_names(options.include_thread_names)
                    .with_writer(log_writer),
            )
            .init();
    }

    keep_log_worker_guard(log_guard);

    Ok(())
}

fn build_log_writer(options: &TracingOptions) -> (BoxMakeWriter, Option<WorkerGuard>) {
    if !options.non_blocking {
        return (BoxMakeWriter::new(std::io::stdout), None);
    }

    let (writer, guard) = tracing_appender::non_blocking::NonBlockingBuilder::default()
        .lossy(options.drop_when_full)
        .buffered_lines_limit(options.non_blocking_buffered_lines.max(1))
        .finish(std::io::stdout());
    (BoxMakeWriter::new(writer), Some(guard))
}

fn keep_log_worker_guard(log_guard: Option<WorkerGuard>) {
    let Some(guard) = log_guard else {
        return;
    };

    if let Err(guard) = TRACING_WORKER_GUARD.set(guard) {
        mem::forget(guard);
    }
}

fn resolve_open_telemetry_options(
    options: &OpenTelemetryOptions,
) -> Result<Option<ResolvedOpenTelemetryOptions>> {
    if !options.enabled {
        return Ok(None);
    }

    let endpoint = options.endpoint.trim();
    if endpoint.is_empty() {
        return Err(anyhow!(
            "openTelemetry.endpoint must be configured when OpenTelemetry is enabled"
        ));
    }

    let protocol = match options.protocol.trim().to_ascii_lowercase().as_str() {
        "" | "grpc" => OpenTelemetryProtocol::Grpc,
        value => {
            return Err(anyhow!(
                "unsupported openTelemetry.protocol `{value}`; only `grpc` is currently supported"
            ));
        }
    };

    Ok(Some(ResolvedOpenTelemetryOptions {
        endpoint: normalized_endpoint(endpoint, options.insecure),
        protocol,
        timeout_ms: options.timeout_ms,
        insecure: options.insecure,
        sample_ratio: options.sample_ratio.clamp(0.0, 1.0),
        service_name: options.service_name.trim().to_string(),
        service_namespace: options.service_namespace.trim().to_string(),
        service_instance_id: options.service_instance_id.trim().to_string(),
        deployment_environment: options.deployment_environment.trim().to_string(),
    }))
}

fn build_open_telemetry_layer<S>(
    options: &ResolvedOpenTelemetryOptions,
) -> Result<tracing_opentelemetry::OpenTelemetryLayer<S, opentelemetry_sdk::trace::Tracer>>
where
    S: tracing::Subscriber + for<'span> tracing_subscriber::registry::LookupSpan<'span>,
{
    let exporter = match options.protocol {
        OpenTelemetryProtocol::Grpc => opentelemetry_otlp::SpanExporter::builder()
            .with_tonic()
            .with_endpoint(options.endpoint.clone())
            .with_timeout(Duration::from_millis(options.timeout_ms))
            .build()?,
    };
    let tracer_provider = SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .with_sampler(Sampler::ParentBased(Box::new(Sampler::TraceIdRatioBased(
            options.sample_ratio,
        ))))
        .with_resource(build_open_telemetry_resource(options))
        .build();
    let tracer = tracer_provider.tracer("ntgw-observability");
    global::set_tracer_provider(tracer_provider);

    Ok(tracing_opentelemetry::layer().with_tracer(tracer))
}

fn build_open_telemetry_resource(options: &ResolvedOpenTelemetryOptions) -> Resource {
    let mut attributes = Vec::new();
    if !options.service_namespace.is_empty() {
        attributes.push(KeyValue::new(
            "service.namespace",
            options.service_namespace.clone(),
        ));
    }
    if !options.service_instance_id.is_empty() {
        attributes.push(KeyValue::new(
            "service.instance.id",
            options.service_instance_id.clone(),
        ));
    }
    if !options.deployment_environment.is_empty() {
        attributes.push(KeyValue::new(
            "deployment.environment",
            options.deployment_environment.clone(),
        ));
    }

    Resource::builder()
        .with_service_name(options.service_name.clone())
        .with_attributes(attributes)
        .build()
}

fn normalized_endpoint(endpoint: &str, insecure: bool) -> String {
    if endpoint.contains("://") {
        endpoint.to_string()
    } else if insecure {
        format!("http://{endpoint}")
    } else {
        format!("https://{endpoint}")
    }
}

#[cfg(test)]
mod tests;
