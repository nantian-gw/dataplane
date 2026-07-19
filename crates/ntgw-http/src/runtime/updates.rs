use std::collections::{BTreeMap, BTreeSet};

use super::{ListenerPlan, PlannedListener};

#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct ListenerUpdatePlan {
    pub(crate) start: Vec<PlannedListener>,
    pub(crate) stop: Vec<String>,
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
    let mut desired_by_bind = desired
        .map(|plan| {
            plan.listeners
                .iter()
                .cloned()
                .map(|listener| (listener.bind.clone(), listener))
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();
    let mut updates = ListenerUpdatePlan::default();

    for (bind, listener) in active {
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
            Some(next) if !finished.contains(bind) && listener.protocol == next.protocol => {}
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
