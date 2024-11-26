use crate::helpers::{test_app, TestApp};
use rstest::*;

#[rstest]
#[tokio::test]
async fn health_check_works(#[future] test_app: TestApp) {
    let client = reqwest::Client::new();

    let response = client
        .get(&format!("{}/health_check", &test_app.await.address))
        .send()
        .await
        .expect("Failed to excute request.");

    assert!(response.status().is_success());
    assert_eq!(Some(0), response.content_length());
}
