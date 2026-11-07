use super::common::SecurityDevice;

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
