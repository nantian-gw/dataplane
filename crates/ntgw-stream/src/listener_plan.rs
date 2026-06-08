use std::collections::{BTreeMap, BTreeSet};

use ntgw_ir::{Listener, Snapshot};

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ListenerPlan {
    pub(crate) listeners: Vec<PlannedListener>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PlannedListener {
    pub(crate) name: String,
    pub(crate) bind: String,
    pub(crate) protocol: StreamProtocol,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct ListenerUpdatePlan {
    pub(crate) start: Vec<PlannedListener>,
    pub(crate) stop: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum StreamProtocol {
    Tcp,
    Udp,
}

pub(crate) fn listener_updates(
    active: &BTreeMap<String, PlannedListener>,
    desired: Option<&ListenerPlan>,
    finished: &BTreeSet<String>,
) -> ListenerUpdatePlan {
    listener_updates_with_force_reload(active, desired, finished, false)
}

pub(crate) fn listener_updates_with_force_reload(
    active: &BTreeMap<String, PlannedListener>,
    desired: Option<&ListenerPlan>,
    finished: &BTreeSet<String>,
    force_reload: bool,
) -> ListenerUpdatePlan {
    let mut desired_by_name = desired
        .map(|plan| {
            plan.listeners
                .iter()
                .cloned()
                .map(|listener| (listener.name.clone(), listener))
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();
    let mut updates = ListenerUpdatePlan::default();

    for (name, listener) in active {
        if force_reload && !finished.contains(name) {
            if let Some(next) = desired_by_name.remove(name) {
                updates.stop.push(name.clone());
                updates.start.push(next);
                continue;
            }
            updates.stop.push(name.clone());
            continue;
        }

        match desired_by_name.remove(name) {
            Some(next)
                if !finished.contains(name)
                    && listener.bind == next.bind
                    && listener.protocol == next.protocol => {}
            Some(next) => {
                updates.stop.push(name.clone());
                updates.start.push(next);
            }
            None => updates.stop.push(name.clone()),
        }
    }

    updates.start.extend(desired_by_name.into_values());
    updates
}

pub(crate) fn build_listener_plan(snapshot: &Snapshot) -> Option<ListenerPlan> {
    let mut listeners = snapshot
        .listeners
        .iter()
        .filter_map(|listener| {
            stream_protocol(&listener.protocol).map(|protocol| PlannedListener {
                name: listener.name.clone(),
                bind: listener_bind_addr(listener),
                protocol,
            })
        })
        .collect::<Vec<_>>();

    listeners.sort_by(|left, right| {
        left.bind
            .cmp(&right.bind)
            .then(left.name.cmp(&right.name))
            .then(left.protocol.cmp(&right.protocol))
    });

    (!listeners.is_empty()).then_some(ListenerPlan { listeners })
}

fn stream_protocol(protocol: &str) -> Option<StreamProtocol> {
    match protocol {
        "LISTENER_PROTOCOL_TCP" | "TCP" => Some(StreamProtocol::Tcp),
        "LISTENER_PROTOCOL_UDP" | "UDP" => Some(StreamProtocol::Udp),
        _ => None,
    }
}

fn listener_bind_addr(listener: &Listener) -> String {
    let address = if listener.address.is_empty() {
        "0.0.0.0"
    } else {
        listener.address.as_str()
    };

    crate::socket_addr(address, listener.port)
}
