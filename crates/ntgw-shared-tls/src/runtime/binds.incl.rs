fn observe_reload_stage_elapsed(
    stage_recorder: Option<&dyn ntgw_observability::ApplyStageRecorder>,
    stage: &str,
    started_at: Instant,
) {
    if let Some(stage_recorder) = stage_recorder {
        stage_recorder.observe_apply_stage_duration(
            stage,
            started_at.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
        );
    }
}

pub(crate) async fn handle_connection(
    bind: Arc<PlannedSharedTlsBind>,
    downstream: TcpStream,
    snapshot: SharedSnapshot,
    http_app: AcceptedHttpApp,
    shutdown: watch::Receiver<bool>,
    _config: ConnectionConfig,
) -> Result<()> {
    let downstream_socket_digest = socket_digest_from_tcp_stream(&downstream);
    let mut downstream = L4Stream::from(downstream);
    downstream.set_socket_digest(downstream_socket_digest);
    let client_hello = peek_client_hello(&mut downstream).await?;
    let server_name = client_hello.server_name.as_deref();
    let (passthrough_match, terminate_match) = select_shared_tls_listeners(
        &snapshot,
        bind.passthrough
            .as_ref()
            .map(|passthrough| passthrough.listener_names.as_slice())
            .unwrap_or_default(),
        bind.terminate
            .as_ref()
            .map(|terminate| terminate.listener_names.as_slice())
            .unwrap_or_default(),
        server_name,
    );

    if let Some(listener_match) = passthrough_match.as_ref() {
        let terminate_has_better_route = terminate_match.as_ref().is_some_and(|candidate| {
            candidate.has_route && candidate.score >= listener_match.score
        });
        if listener_match.has_route || !terminate_has_better_route {
            return proxy_passthrough(
                downstream,
                &snapshot,
                listener_match.listener_name.as_str(),
                server_name,
            )
            .await;
        }
    }

    let terminate = bind
        .terminate
        .as_ref()
        .ok_or_else(|| anyhow!("no terminate surface for bind {}", bind.bind))?;
    if bind.passthrough.is_some() && !terminate_match.as_ref().is_some_and(|item| item.has_route) {
        return Err(anyhow!(
            "no shared tls listener route matched SNI {} on bind {}",
            server_name.unwrap_or_default(),
            bind.bind
        ));
    }
    if bind.passthrough.is_none() && terminate_match.is_none() {
        return Err(anyhow!(
            "no terminate listener matched SNI {} on bind {}",
            server_name.unwrap_or_default(),
            bind.bind
        ));
    }
    let tls_stream = terminate_tls(downstream, terminate).await?;
    if let Some(listener_match) = terminate_match.as_ref()
        && terminate_match_uses_tls_stream_route(
            &snapshot,
            listener_match.listener_name.as_str(),
            server_name,
        ) {
            return proxy_terminated_stream(
                tls_stream,
                &snapshot,
                listener_match.listener_name.as_str(),
                server_name,
            )
            .await;
        }
    process_accepted_stream(http_app, Box::new(tls_stream), shutdown).await
}

#[cfg(unix)]
fn socket_digest_from_tcp_stream(stream: &TcpStream) -> SocketDigest {
    SocketDigest::from_raw_fd(stream.as_raw_fd())
}

#[cfg(windows)]
fn socket_digest_from_tcp_stream(stream: &TcpStream) -> SocketDigest {
    SocketDigest::from_raw_socket(stream.as_raw_socket())
}

#[derive(Default)]
struct SharedTlsBindSet {
    tasks: BTreeMap<String, SharedTlsBindTask>,
}

struct SharedTlsBindTask {
    bind: PlannedSharedTlsBind,
    shutdown: watch::Sender<bool>,
    join: JoinHandle<()>,
}

#[derive(Debug, Default, PartialEq, Eq)]
struct BindReplaceResult {
    failures: Vec<RuntimeListenerFailure>,
    started_listeners: Vec<String>,
    retained_listeners: Vec<String>,
}

impl SharedTlsBindSet {
    fn needs_reload(&self, desired: Option<&ListenerPlan>, force_reload: bool) -> bool {
        let updates = listener_updates(
            &self.active_plan(),
            desired,
            &self.finished_tasks(),
            force_reload,
        );
        !updates.start.is_empty() || !updates.stop.is_empty()
    }

    async fn replace(
        &mut self,
        plan: Option<ListenerPlan>,
        snapshot: SharedSnapshot,
        http_app: AcceptedHttpApp,
        shutdown: watch::Receiver<bool>,
        force_reload: bool,
    ) -> BindReplaceResult {
        let updates = listener_updates(
            &self.active_plan(),
            plan.as_ref(),
            &self.finished_tasks(),
            force_reload,
        );
        if updates.start.is_empty() && updates.stop.is_empty() {
            return BindReplaceResult::default();
        }

        let mut failures = Vec::new();
        let mut started_listeners = Vec::new();
        let mut restarted_binds = BTreeSet::new();

        for bind in updates.stop {
            restarted_binds.insert(bind.clone());
            if let Some(task) = self.tasks.remove(&bind) {
                stop_bind_task(bind.as_str(), task).await;
            }
        }

        for bind in updates.start {
            let bind_name = bind.bind.clone();
            match spawn_bind_task(
                bind.clone(),
                snapshot.clone(),
                http_app.clone(),
                shutdown.clone(),
            )
            .await
            {
                Ok(task) => {
                    started_listeners.extend(bind_listener_names(&bind));
                    self.tasks.insert(bind_name, task);
                }
                Err(err) => {
                    let message = err.to_string();
                    warn!(bind = %bind.bind, error = %err, "failed to start shared tls bind");
                    failures.extend(bind_listener_names(&bind).into_iter().map(|listener| {
                        RuntimeListenerFailure {
                            listener,
                            message: message.clone(),
                        }
                    }));
                }
            }
        }

        let started_names = started_listeners.iter().cloned().collect::<BTreeSet<_>>();
        let retained_listeners = self
            .tasks
            .values()
            .filter(|task| !restarted_binds.contains(task.bind.bind.as_str()))
            .flat_map(|task| bind_listener_names(&task.bind))
            .filter(|listener| !started_names.contains(listener))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();

        BindReplaceResult {
            failures,
            started_listeners,
            retained_listeners,
        }
    }

    fn active_plan(&self) -> BTreeMap<String, PlannedSharedTlsBind> {
        self.tasks
            .iter()
            .map(|(bind, task)| (bind.clone(), task.bind.clone()))
            .collect()
    }

    fn active_listener_names(&self) -> Vec<String> {
        self.tasks
            .values()
            .flat_map(|task| bind_listener_names(&task.bind))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    fn finished_tasks(&self) -> BTreeSet<String> {
        self.tasks
            .iter()
            .filter(|(_, task)| task.join.is_finished())
            .map(|(bind, _)| bind.clone())
            .collect()
    }

    async fn shutdown_all(&mut self) {
        for (bind, task) in std::mem::take(&mut self.tasks) {
            stop_bind_task(bind.as_str(), task).await;
        }
    }
}

#[derive(Default)]
struct BindUpdatePlan {
    start: Vec<PlannedSharedTlsBind>,
    stop: Vec<String>,
}

fn listener_updates(
    active: &BTreeMap<String, PlannedSharedTlsBind>,
    desired: Option<&ListenerPlan>,
    finished: &BTreeSet<String>,
    force_reload: bool,
) -> BindUpdatePlan {
    let mut desired_by_bind = desired.map(|plan| plan.binds.clone()).unwrap_or_default();
    let mut updates = BindUpdatePlan::default();

    for (bind, active_bind) in active {
        if force_reload && !finished.contains(bind) {
            if let Some(next) = desired_by_bind.remove(bind) {
                updates.stop.push(bind.clone());
                updates.start.push(next);
                continue;
            }
            updates.stop.push(bind.clone());
            continue;
        }

        match desired_by_bind.remove(bind) {
            Some(next) if !finished.contains(bind) && active_bind == &next => {}
            Some(next) => {
                updates.stop.push(bind.clone());
                updates.start.push(next);
            }
            None => updates.stop.push(bind.clone()),
        }
    }

    updates.start.extend(desired_by_bind.into_values());
    updates
}

async fn spawn_bind_task(
    bind: PlannedSharedTlsBind,
    snapshot: SharedSnapshot,
    http_app: AcceptedHttpApp,
    shutdown: watch::Receiver<bool>,
) -> Result<SharedTlsBindTask> {
    let listener = bind_tcp_listener(&bind.bind).await?;
    let bind_addr = bind.bind.clone();
    let bind_plan = bind.clone();
    let (task_shutdown, mut task_shutdown_rx) = watch::channel(false);
    let join = tokio::spawn(async move {
        info!(bind = %bind_addr, "shared tls bind started");
        loop {
            tokio::select! {
                _ = task_shutdown_rx.changed() => {
                    info!(bind = %bind_addr, "shared tls bind stopping");
                    return;
                }
                accepted = listener.accept() => {
                    let (stream, peer) = match accepted {
                        Ok(value) => value,
                        Err(err) => {
                            warn!(bind = %bind_addr, error = %err, "shared tls accept loop failed");
                            return;
                        }
                    };
                    let task_bind = Arc::new(bind_plan.clone());
                    let task_snapshot = snapshot.clone();
                    let task_http_app = http_app.clone();
                    let task_shutdown = shutdown.clone();
                    tokio::spawn(async move {
                        if let Err(err) = handle_connection(
                            task_bind,
                            stream,
                            task_snapshot,
                            task_http_app,
                            task_shutdown,
                            ConnectionConfig,
                        )
                        .await
                        {
                            warn!(peer = %peer, error = %err, "shared tls connection failed");
                        }
                    });
                }
            }
        }
    });

    Ok(SharedTlsBindTask {
        bind,
        shutdown: task_shutdown,
        join,
    })
}

async fn bind_tcp_listener(bind: &str) -> Result<TcpListener> {
    if !bind.starts_with('[') {
        return TcpListener::bind(bind)
            .await
            .with_context(|| format!("bind shared tls listener {bind}"));
    }

    let addr: SocketAddr = bind
        .parse()
        .with_context(|| format!("parse shared tls listener address {bind}"))?;
    let socket = Socket::new(Domain::IPV6, Type::STREAM, Some(Protocol::TCP))
        .with_context(|| format!("create shared tls listener socket {bind}"))?;
    socket
        .set_only_v6(true)
        .with_context(|| format!("mark shared tls listener {bind} as ipv6-only"))?;
    socket
        .bind(&addr.into())
        .with_context(|| format!("bind shared tls listener {bind}"))?;
    socket
        .listen(1024)
        .with_context(|| format!("listen shared tls listener {bind}"))?;

    let std_listener: std::net::TcpListener = socket.into();
    std_listener
        .set_nonblocking(true)
        .with_context(|| format!("set shared tls listener {bind} nonblocking"))?;
    TcpListener::from_std(std_listener)
        .with_context(|| format!("adopt shared tls listener {bind} into tokio"))
}

async fn stop_bind_task(bind: &str, task: SharedTlsBindTask) {
    let _ = task.shutdown.send(true);
    if let Err(err) = task.join.await {
        error!(bind = %bind, error = %err, "failed to join shared tls bind");
    }
}

fn desired_listener_plan(
    snapshot: &Snapshot,
    runtime: &RuntimeOptions,
) -> Result<Option<ListenerPlan>> {
    if !snapshot.listeners.iter().any(shared_tls_listener_protocol) {
        return Ok(None);
    }

    let plan = build_listener_plan(snapshot, runtime)?;
    if plan.binds.is_empty() {
        return Ok(None);
    }

    Ok(Some(plan))
}

fn shared_tls_listener_protocol(listener: &ntgw_ir::Listener) -> bool {
    matches!(
        listener.protocol.as_str(),
        "LISTENER_PROTOCOL_HTTPS"
            | "LISTENER_PROTOCOL_TLS_PASSTHROUGH"
            | "HTTPS"
            | "TLS"
            | "TLS_PASSTHROUGH"
    )
}
