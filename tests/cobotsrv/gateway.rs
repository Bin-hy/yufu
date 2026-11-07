use super::common::SecurityDevice;

#[test]
fn security_t02_injected_clock_controls_budget_and_freshness() {
    use super::common::SecurityTestClock;
    use auth::security::{
        budget::SecurityBudget, credential::DecisionValidity, error::SecurityError,
    };
    use std::{sync::Arc, time::Duration};

    let clock = Arc::new(SecurityTestClock::new(100_000));
    let budget = SecurityBudget::new(clock.clone()).unwrap();
    let validity = DecisionValidity::from_query_start(100_000, 200_000, None, None)
        .unwrap()
        .bind_to_clock(clock.clone())
        .unwrap();
    clock.advance(1500);
    assert_eq!(budget.remaining().unwrap(), Duration::from_millis(500));
    assert_eq!(validity.remaining().unwrap(), Duration::from_millis(2500));
    clock.set_wall_time(99_000);
    assert_eq!(budget.remaining().unwrap(), Duration::from_millis(500));
    assert_eq!(validity.remaining().unwrap(), Duration::from_millis(2500));
    clock.advance(500);
    assert_eq!(budget.remaining(), Err(SecurityError::DeadlineExceeded));
    clock.set_synchronized(false);
    assert_eq!(
        validity.remaining(),
        Err(SecurityError::DependencyUnavailable)
    );
}

#[tokio::test]
async fn security_t02_device_counts_only_received_requests() {
    let device = SecurityDevice::start().await;
    assert_eq!(device.request_count(), 0);
    let response = reqwest::Client::new()
        .post(format!("{}/execute", device.base))
        .send()
        .await
        .unwrap();
    assert!(response.status().is_success());
    assert_eq!(device.request_count(), 1);
}
