use crate::helpers::{test_app, TestApp};
use fake::faker::internet::en::SafeEmail;
use fake::faker::name::en::Name;
use fake::Fake;
use rstest::*;
use wiremock::matchers::{any, method, path};
use wiremock::{Mock, ResponseTemplate};

#[rstest]
#[tokio::test]
async fn newsletters_are_not_delivered_to_unconfirmed_subscribers(#[future] test_app: TestApp) {
    let app = test_app.await;
    create_unconfirmed_subscriber(&app).await;

    Mock::given(any())
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&app.email_server)
        .await;

    let newsletter_request_body = serde_json::json!({
        "title": "Newsletter title",
        "content": "<p>Newsletter body as HTML</p>",
    });
    let response = app.post_newsletters(newsletter_request_body).await;

    assert_eq!(response.status().as_u16(), 200);
}

#[rstest]
#[tokio::test]
async fn newsletters_are_delivered_to_confirmed_subscribers(#[future] test_app: TestApp) {
    let app = test_app.await;
    create_confirmed_subscriber(&app).await;

    Mock::given(any())
        .respond_with(ResponseTemplate::new(200))
        .expect(1..)
        .mount(&app.email_server)
        .await;

    let newsletter_request_body = serde_json::json!({
        "title": "Newsletter title",
        "content": "<p>Newsletter body as HTML</p>",
    });
    let response = app.post_newsletters(newsletter_request_body).await;

    assert_eq!(response.status().as_u16(), 200);
}

#[rstest]
#[case(
    serde_json::json!({"content": "<p>Newsletter body as HTML</p>"}),
    "missing content",
)]
#[case(serde_json::json!({"title": "Newsletter!"}),"missing content",)]
#[tokio::test]
async fn newsletters_returns_400_for_invalid_data(
    #[future] test_app: TestApp,
    #[case] invalid_data: serde_json::Value,
    #[case] error_message: String,
) {
    let app = test_app.await;
    let response = app.post_newsletters(invalid_data).await;
    assert_eq!(
        400,
        response.status().as_u16(),
        "The API did not fail with 400 Bad Request when the payload was {}.",
        error_message
    )
}

async fn create_unconfirmed_subscriber(app: &TestApp) -> reqwest::Url {
    let body = format!(
        "name={}&email={}",
        Name().fake::<String>(),
        SafeEmail().fake::<String>()
    );
    let _mock_guard = Mock::given(path("/v1.0/me/messages"))
        .and(method("POST"))
        .respond_with(ResponseTemplate::new(200))
        .named("Create unconfirmed subscriber")
        .expect(1)
        .mount_as_scoped(&app.email_server)
        .await;

    app.post_subscriptions(body.into())
        .await
        .error_for_status()
        .unwrap();

    let email_request = &app
        .email_server
        .received_requests()
        .await
        .unwrap()
        .pop()
        .unwrap();
    app.get_confirmation_link(&email_request)
}

async fn create_confirmed_subscriber(app: &TestApp) {
    let confirmation_link = create_unconfirmed_subscriber(app).await;
    reqwest::get(confirmation_link)
        .await
        .unwrap()
        .error_for_status()
        .unwrap();
}
