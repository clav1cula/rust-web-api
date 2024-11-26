use crate::helpers::{test_app, TestApp};
use fake::faker::internet::en::SafeEmail;
use fake::faker::name::en::Name;
use fake::Fake;
use rstest::*;
use wiremock::matchers::{method, path};
use wiremock::{Mock, ResponseTemplate};

#[rstest]
#[tokio::test]
async fn subscribe_returns_a_200_for_valid_form_data(#[future] test_app: TestApp) {
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

    let response = app.post_subscriptions(body.into()).await;
    assert_eq!(200, response.status().as_u16());
}

#[rstest]
#[tokio::test]
async fn subscribe_persists_the_new_subscriber(#[future] test_app: TestApp) {
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

    let saved = sqlx::query!(
        "SELECT email, name, status FROM subscriptions WHERE email=$1",
        fake_email
    )
    .fetch_one(&app.db_pool)
    .await
    .expect("Failed to fetch saved subscription.");

    assert_eq!(saved.email, fake_email);
    assert_eq!(saved.name, fake_name);
    assert_eq!(saved.status, "pending_confirmation");
}

#[rstest]
#[case(format!("name={}",Name().fake::<String>()), "missing the email")]
#[case(format!("email={}",SafeEmail().fake::<String>()), "missing the name")]
#[case("", "missing both name and email")]
#[case(format!("name=&email={}",SafeEmail().fake::<String>()), "empty name")]
#[case(format!("name={}&email=",Name().fake::<String>()), "empty email")]
#[case(format!("name={}&email=definitely-not-an-email",
    Name().fake::<String>()), "invalid email")]
#[tokio::test]
async fn subscribe_400_rt(
    #[future] test_app: TestApp,
    #[case] invalid_body: String,
    #[case] error_message: String,
) {
    let app = test_app.await;
    let response = app.post_subscriptions(invalid_body.into()).await;

    assert_eq!(
        400,
        response.status().as_u16(),
        "The API did not fail with 400 Bad Request when the payload was {error_message}."
    );
}

#[rstest]
#[tokio::test]
async fn subscribe_sends_a_confirmation_email_for_valid_data(#[future] test_app: TestApp) {
    let app = test_app.await;
    let body = format!(
        "name={}&email={}",
        Name().fake::<String>(),
        SafeEmail().fake::<String>()
    );

    Mock::given(path("/v1.0/me/messages"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&app.email_server)
        .await;

    app.post_subscriptions(body.into()).await;
}

#[rstest]
#[tokio::test]
async fn subscribe_sends_a_confirmation_email_with_a_link(#[future] test_app: TestApp) {
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
    let confirmation_links = app.get_confirmation_link(&email_request);
    assert!(confirmation_links.as_str().starts_with("http://"))
}
