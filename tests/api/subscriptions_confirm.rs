use crate::helpers::{test_app, TestApp};
use fake::faker::internet::en::SafeEmail;
use fake::faker::name::en::Name;
use fake::Fake;
use rstest::*;
use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

#[rstest]
#[tokio::test]
async fn confirmations_without_token_are_rejected_with_a_400(#[future] test_app: TestApp) {
    let app = test_app.await;
    let response = reqwest::get(&format!("{}/subscriptions/confirm", app.address))
        .await
        .unwrap();
    assert_eq!(response.status().as_u16(), 400);
}

#[rstest]
#[tokio::test]
async fn the_link_returned_by_subscribe_returns_a_200_if_called(#[future] test_app: TestApp) {
    let app = test_app.await;
    let body = format!(
        "name={}&email={}",
        Name().fake::<String>(),
        SafeEmail().fake::<String>()
    );

    Mock::given(path("/v1.0/me/messages"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&app.email_server)
        .await;

    app.post_subscriptions(body.into()).await;
    let email_request = &app.email_server.received_requests().await.unwrap()[0];
    let confirmation_link = app.get_confirmation_link(&email_request);
    let response = reqwest::get(confirmation_link).await.unwrap();
    assert_eq!(response.status().as_u16(), 200);
}

#[rstest]
#[tokio::test]
async fn clicking_on_the_confirmation_link_confirms_a_subscriber(#[future] test_app: TestApp) {
    let app = test_app.await;
    let fake_name: String = Name().fake();
    let fake_email: String = SafeEmail().fake();
    let body = format!("name={fake_name}&email={fake_email}");

    Mock::given(path("/v1.0/me/messages"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&app.email_server)
        .await;

    app.post_subscriptions(body.into()).await;
    let email_request = &app.email_server.received_requests().await.unwrap()[0];
    let confirmation_link = app.get_confirmation_link(&email_request);
    reqwest::get(confirmation_link)
        .await
        .unwrap()
        .error_for_status()
        .unwrap();

    let saved = sqlx::query!(
        "SELECT email, name, status FROM subscriptions WHERE email = $1",
        fake_email
    )
    .fetch_one(&app.db_pool)
    .await
    .expect("Failed to fetch saved subscription.");
    assert_eq!(saved.email, fake_email);
    assert_eq!(saved.name, fake_name);
    assert_eq!(saved.status, "confirmed");
}
