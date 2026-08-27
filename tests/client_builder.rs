use std::time::Duration;

use pl_client_api_rs::{Client, Error};

#[test]
fn builder_normalizes_a_custom_base_url() {
    let client = Client::builder()
        .api_base_url("http://127.0.0.1:3000/api")
        .timeout(Duration::from_secs(1))
        .plar_version(2501)
        .device_id("test-device")
        .language("English")
        .build()
        .unwrap();

    assert_eq!(client.api_base_url().as_str(), "http://127.0.0.1:3000/api/");
    assert_eq!(client.plar_version(), 2501);
    assert_eq!(client.device_id(), "test-device");
    assert_eq!(client.language(), "English");
}

#[test]
fn builder_rejects_an_invalid_base_url() {
    let result = Client::builder().api_base_url("not a URL").build();

    assert!(matches!(result, Err(Error::InvalidBaseUrl { .. })));
}
