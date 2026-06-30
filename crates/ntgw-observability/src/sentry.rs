use anyhow::Result;

#[derive(Debug, Clone)]
pub struct SentryOptions {
    pub enabled: bool,
    pub dsn: String,
    pub environment: String,
    pub sample_rate: f32,
    pub traces_sample_rate: f32,
    pub attach_stacktrace: bool,
    pub send_default_pii: bool,
    pub debug: bool,
}

impl Default for SentryOptions {
    fn default() -> Self {
        Self {
            enabled: false,
            dsn: String::new(),
            environment: String::new(),
            sample_rate: 1.0,
            traces_sample_rate: 0.01,
            attach_stacktrace: true,
            send_default_pii: false,
            debug: false,
        }
    }
}

pub struct SentryGuard {
    _guard: Option<sentry::ClientInitGuard>,
}

pub fn init_sentry(options: &SentryOptions, release: &str) -> Result<SentryGuard> {
    if !options.enabled || options.dsn.is_empty() {
        return Ok(SentryGuard { _guard: None });
    }

    let guard = sentry::init((options.dsn.as_str(), sentry::ClientOptions {
        release: Some(release.to_owned().into()),
        environment: Some(options.environment.clone().into()),
        sample_rate: options.sample_rate,
        traces_sample_rate: options.traces_sample_rate,
        attach_stacktrace: options.attach_stacktrace,
        send_default_pii: options.send_default_pii,
        debug: options.debug,
        ..Default::default()
    }));

    Ok(SentryGuard {
        _guard: Some(guard),
    })
}
