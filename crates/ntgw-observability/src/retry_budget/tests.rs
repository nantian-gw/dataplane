use super::{RetryBudgetController, RetryBudgetOptions};

#[test]
fn retry_budget_consumes_tokens_and_refills_from_retryable_requests() {
    let budget = RetryBudgetController::new(RetryBudgetOptions {
        enabled: true,
        ratio_percent: 50,
        burst: 2,
    });

    assert!(budget.try_acquire_retry());
    assert!(budget.try_acquire_retry());
    assert!(!budget.try_acquire_retry());

    budget.observe_retryable_request();
    assert!(!budget.try_acquire_retry());

    budget.observe_retryable_request();
    assert!(budget.try_acquire_retry());

    let snapshot = budget.snapshot();
    assert_eq!(snapshot.retry_allowed_total, 3);
    assert_eq!(snapshot.retry_rejected_total, 2);
    assert_eq!(snapshot.retryable_requests_observed_total, 2);
}

#[test]
fn retry_budget_refill_stays_capped_by_burst_capacity() {
    let budget = RetryBudgetController::new(RetryBudgetOptions {
        enabled: true,
        ratio_percent: 100,
        burst: 1,
    });

    assert!(budget.try_acquire_retry());
    assert!(!budget.try_acquire_retry());

    budget.observe_retryable_request();
    budget.observe_retryable_request();

    assert!(budget.try_acquire_retry());
    assert!(!budget.try_acquire_retry());

    let snapshot = budget.snapshot();
    assert_eq!(snapshot.burst, 1);
    assert_eq!(snapshot.available_tokens, 0);
}
