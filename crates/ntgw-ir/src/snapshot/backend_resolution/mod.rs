mod resolution;
#[cfg(test)]
mod tests;

use super::super::*;
use std::borrow::Cow;

// ---------------------------------------------------------------------------
// Helper types
// ---------------------------------------------------------------------------

pub(crate) struct BackendSelectionCandidate<'a> {
    pub(super) cluster: &'a BackendCluster,
    pub(super) backend_ref: &'a BackendRef,
    pub(super) backend_name: Cow<'a, str>,
}
#[derive(Default)]
pub(crate) struct BackendCandidateVisit<'a> {
    pub(super) candidate_count: usize,
    pub(super) total_weight: u64,
    pub(super) saw_invalid_refs: bool,
    pub(super) saw_unhealthy_backend: bool,
    pub(super) effective_load_balancing: Option<&'a LoadBalancingPolicy>,
    pub(super) load_balancing_mismatch: bool,
}

impl<'a> BackendCandidateVisit<'a> {
    pub(super) fn observe_candidate(
        &mut self,
        backend_ref: &BackendRef,
        load_balancing: Option<&'a LoadBalancingPolicy>,
    ) {
        self.candidate_count += 1;
        self.total_weight += backend_ref.weight as u64;

        if self.load_balancing_mismatch {
            return;
        }

        let Some(load_balancing) = load_balancing else {
            self.effective_load_balancing = None;
            self.load_balancing_mismatch = true;
            return;
        };

        match self.effective_load_balancing {
            Some(current) if current != load_balancing => {
                self.effective_load_balancing = None;
                self.load_balancing_mismatch = true;
            }
            Some(_) => {}
            None => self.effective_load_balancing = Some(load_balancing),
        }
    }

    pub(super) fn error(&self) -> Option<BackendSelectionError> {
        if self.saw_unhealthy_backend {
            Some(BackendSelectionError::NoHealthyBackends)
        } else if self.saw_invalid_refs {
            Some(BackendSelectionError::InvalidBackendRefs)
        } else {
            None
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct EndpointSelectionAvailability {
    pub(super) count: usize,
    pub(super) include_passive_ejected: bool,
}

#[derive(Clone, Copy)]
pub(super) enum EndpointAvailability {
    Primary,
    PassiveLastResort,
    Unavailable,
}

// ---------------------------------------------------------------------------
// Free functions
// ---------------------------------------------------------------------------

pub(super) fn spread_weighted_target(ticket: u64, total_weight: u64) -> u64 {
    if total_weight <= 1 {
        return 0;
    }

    ticket.wrapping_mul(spread_weighted_stride(total_weight)) % total_weight
}

fn spread_weighted_stride(total_weight: u64) -> u64 {
    if total_weight <= 1 {
        return 1;
    }

    const PHI_NUMERATOR: u128 = 618_033_988_749_895;
    const PHI_DENOMINATOR: u128 = 1_000_000_000_000_000;

    let ideal =
        (((total_weight as u128) * PHI_NUMERATOR + (PHI_DENOMINATOR / 2)) / PHI_DENOMINATOR) as u64;
    let ideal = ideal.clamp(1, total_weight - 1);

    for delta in 0..total_weight {
        if let Some(candidate) = ideal.checked_sub(delta)
            && candidate > 0
            && gcd(candidate, total_weight) == 1
        {
            return candidate;
        }
        if let Some(candidate) = ideal.checked_add(delta)
            && candidate < total_weight
            && gcd(candidate, total_weight) == 1
        {
            return candidate;
        }
    }

    1
}

fn gcd(mut left: u64, mut right: u64) -> u64 {
    while right != 0 {
        let next = left % right;
        left = right;
        right = next;
    }
    left
}

pub(super) fn backend_ref_is_routable(backend_ref: &BackendRef) -> bool {
    if backend_ref
        .metadata
        .get(BACKEND_REF_META_VALID)
        .is_some_and(|value| value == "false")
    {
        return false;
    }
    matches!(
        (backend_ref.group.as_str(), backend_ref.kind.as_str()),
        ("", "") | ("", "Service") | ("multicluster.x-k8s.io", "ServiceImport")
    )
}

impl Snapshot {
    pub(super) fn should_mirror(&self, filter: &RequestMirrorFilter) -> bool {
        let (numerator, denominator) = match &filter.fraction {
            Some(fraction) => (fraction.numerator, fraction.denominator.max(1)),
            None => (filter.percent.unwrap_or(100), 100),
        };

        if numerator == 0 {
            return false;
        }
        if numerator >= denominator {
            return true;
        }

        (self.selection_state.next_mirror_ticket() % denominator as u64) < numerator as u64
    }
}
