pub(crate) fn classify_listener_convergence_overview(
    awaiting_current_attempt_count: usize,
    current_attempt_blocked_count: usize,
    serving_drift_count: usize,
) -> (&'static str, &'static str) {
    let active_categories = [
        awaiting_current_attempt_count > 0,
        current_attempt_blocked_count > 0,
        serving_drift_count > 0,
    ]
    .into_iter()
    .filter(|active| *active)
    .count();

    let primary_signal = match active_categories {
        0 => "none",
        1 if current_attempt_blocked_count > 0 => "current-attempt-blocked",
        1 if awaiting_current_attempt_count > 0 => "awaiting-current-attempt",
        1 => "serving-last-good",
        _ => "mixed",
    };

    let severity = if current_attempt_blocked_count > 0 {
        "critical"
    } else if active_categories > 0 {
        "warning"
    } else {
        "ok"
    };

    (severity, primary_signal)
}

pub(crate) fn classify_listener_failure_recovery_overview(
    current_snapshot_failure_count: usize,
    historical_failure_count: usize,
) -> (&'static str, &'static str) {
    let primary_signal = match (
        current_snapshot_failure_count > 0,
        historical_failure_count > 0,
    ) {
        (false, false) => "none",
        (true, false) => "current-snapshot-unrecovered",
        (false, true) => "historical-unrecovered",
        (true, true) => "mixed",
    };

    let severity = if current_snapshot_failure_count > 0 {
        "critical"
    } else if historical_failure_count > 0 {
        "warning"
    } else {
        "ok"
    };

    (severity, primary_signal)
}

pub(crate) fn classify_listener_attention_overview(
    pending_count: usize,
    rejected_count: usize,
    stale_count: usize,
    unrecovered_failure_count: usize,
) -> (&'static str, &'static str) {
    let active_categories = [
        pending_count > 0,
        rejected_count > 0,
        stale_count > 0,
        unrecovered_failure_count > 0,
    ]
    .into_iter()
    .filter(|active| *active)
    .count();

    let primary_signal = match active_categories {
        0 => "none",
        1 if rejected_count > 0 => "rejected",
        1 if unrecovered_failure_count > 0 => "unrecovered-failure",
        1 if pending_count > 0 => "pending",
        1 => "stale",
        _ => "mixed",
    };

    let severity = if rejected_count > 0 || unrecovered_failure_count > 0 {
        "critical"
    } else if active_categories > 0 {
        "warning"
    } else {
        "ok"
    };

    (severity, primary_signal)
}

pub(crate) fn severity_to_level(severity: &str) -> u64 {
    match severity {
        "ok" => 0,
        "warning" => 1,
        "critical" => 2,
        _ => 0,
    }
}

pub(super) fn severity_from_level(level: u64) -> &'static str {
    match level {
        0 => "ok",
        1 => "warning",
        2 => "critical",
        _ => "ok",
    }
}

pub(super) fn recommended_listener_convergence_filter(primary_signal: &str) -> &'static str {
    match primary_signal {
        "awaiting-current-attempt" => "attemptProgress=awaiting-current",
        "current-attempt-blocked" => "attemptProgress=blocked-current",
        "serving-last-good" => "servingSnapshot=last-good",
        "mixed" => "attentionRequired=true",
        _ => "",
    }
}

pub(super) fn recommended_listener_convergence_reason(primary_signal: &str) -> &'static str {
    match primary_signal {
        "awaiting-current-attempt" => {
            "inspect listeners that have not yet attempted the active snapshot"
        }
        "current-attempt-blocked" => {
            "inspect listeners that already attempted the active snapshot but remain pending or rejected"
        }
        "serving-last-good" => {
            "inspect listeners still serving the last-good snapshot instead of the active snapshot"
        }
        "mixed" => {
            "inspect listeners currently requiring attention across multiple convergence states"
        }
        _ => "",
    }
}

pub(super) fn recommended_listener_failure_recovery_filter(primary_signal: &str) -> &'static str {
    match primary_signal {
        "current-snapshot-unrecovered" => "unrecoveredFailureAge=current",
        "historical-unrecovered" => "unrecoveredFailureAge=historical",
        "mixed" => "hasEverFailed=true&recoveredFromFailure=false",
        _ => "",
    }
}

pub(super) fn recommended_listener_attention_filter(primary_signal: &str) -> &'static str {
    match primary_signal {
        "pending" => "attentionReason=pending",
        "rejected" => "attentionReason=rejected",
        "stale" => "attentionReason=stale",
        "unrecovered-failure" => "attentionReason=unrecovered_failure",
        "mixed" => "attentionRequired=true",
        _ => "",
    }
}

pub(super) fn recommended_listener_convergence_count(
    primary_signal: &str,
    awaiting_current_attempt_count: usize,
    current_attempt_blocked_count: usize,
    serving_drift_count: usize,
    attention_required_count: usize,
) -> usize {
    match primary_signal {
        "awaiting-current-attempt" => awaiting_current_attempt_count,
        "current-attempt-blocked" => current_attempt_blocked_count,
        "serving-last-good" => serving_drift_count,
        "mixed" => attention_required_count,
        _ => 0,
    }
}

pub(super) fn recommended_listener_attention_reason(primary_signal: &str) -> &'static str {
    match primary_signal {
        "pending" => "inspect listeners currently marked pending",
        "rejected" => "inspect listeners currently rejected on the active snapshot",
        "stale" => "inspect listeners still serving a stale last-good snapshot",
        "unrecovered-failure" => {
            "inspect listeners that have not yet recovered from observed failures"
        }
        "mixed" => {
            "inspect listeners currently requiring operator attention across multiple categories"
        }
        _ => "",
    }
}

pub(super) fn recommended_listener_failure_recovery_reason(primary_signal: &str) -> &'static str {
    match primary_signal {
        "current-snapshot-unrecovered" => {
            "inspect listeners whose latest unrecovered failure belongs to the active snapshot"
        }
        "historical-unrecovered" => {
            "inspect listeners carrying unrecovered failures from older snapshots"
        }
        "mixed" => "inspect listeners that have failed and are not yet recovered",
        _ => "",
    }
}

pub(super) fn recommended_listener_attention_count(
    primary_signal: &str,
    pending_count: usize,
    rejected_count: usize,
    stale_count: usize,
    unrecovered_failure_count: usize,
    required_count: usize,
) -> usize {
    match primary_signal {
        "pending" => pending_count,
        "rejected" => rejected_count,
        "stale" => stale_count,
        "unrecovered-failure" => unrecovered_failure_count,
        "mixed" => required_count,
        _ => 0,
    }
}

pub(super) fn recommended_listener_failure_recovery_count(
    primary_signal: &str,
    current_snapshot_failure_count: usize,
    historical_failure_count: usize,
    unrecovered_count: usize,
) -> usize {
    match primary_signal {
        "current-snapshot-unrecovered" => current_snapshot_failure_count,
        "historical-unrecovered" => historical_failure_count,
        "mixed" => unrecovered_count,
        _ => 0,
    }
}

pub(super) fn recommended_listener_status_path(filter: &str) -> String {
    if filter.is_empty() {
        String::new()
    } else {
        format!("/v1/listener-statuses?{filter}")
    }
}
