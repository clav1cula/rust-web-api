use rstest::*;
use sqlx::PgPool;
use std::net::TcpListener;
use zero2prod::telemetry::{get_subscriber, init_subscriber};
use zero2prod::{
    configuration::{get_configuration, Settings},
    startup::run,
};

struct TestApp {
    address: String,
    db_pool: PgPool,
}

#[fixture]
#[once]
fn configurations() -> Settings {
    if std::env::var("TEST_LOG").is_ok() {
        let subscriber = get_subscriber("test".into(), "debug".into(), std::io::stdout);
        init_subscriber(subscriber);
    } else {
        let subscriber = get_subscriber("test".into(), "debug".into(), std::io::sink);
        init_subscriber(subscriber);
    }
    get_configuration().expect("Failed to read configuration")
}

#[fixture]
async fn test_app(configurations: &Settings) -> TestApp {
    let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind random port");
    let port = listener.local_addr().unwrap().port();
    let address = format!("http://127.0.0.1:{port}");

    let db_pool = PgPool::connect_with(configurations.database.pg_connection())
        .await
        .expect("Failed to connect to Postgres.");

    let server = run(listener, db_pool.clone()).expect("Failed to bind address");
    let _ = tokio::spawn(server);

    TestApp { address, db_pool }
}

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

#[rstest]
#[tokio::test]
async fn subscribe_200(#[future] test_app: TestApp) {
    let app = test_app.await;
    let client = reqwest::Client::new();

    let body = "name=Arya%20Stark&email=arya%40gmail.com";
    let response = client
        .post(&format!("{}/subscriptions", &app.address))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .expect("Failed to execute request.");

    assert_eq!(200, response.status().as_u16());

    let saved = sqlx::query!("select email, name from subscriptions",)
        .fetch_one(&app.db_pool)
        .await
        .expect("Failed to fetch saved subscription.");

    assert_eq!(saved.email, "arya@gmail.com");
    assert_eq!(saved.name, "Arya Stark");

    _ = sqlx::query!(
        "delete from subscriptions where email =$1",
        "arya@gmail.com"
    )
    .execute(&app.db_pool)
    .await
}

#[rstest]
#[case("name=Arya%20Stark", "missing the email")]
#[case("email=arya%40gmail.com", "missing the name")]
#[case("", "missing both name and email")]
#[case("name=&email=arya%40gmail.com", "empty name")]
#[case("name=Arya%20Stark&email=", "empty email")]
#[case("name=Arya%20Stark&email=definitely-not-an-email", "invalid email")]
#[tokio::test]
async fn subscribe_400_rt(
    #[future] test_app: TestApp,
    #[case] invalid_body: String,
    #[case] error_message: String,
) {
    let client = reqwest::Client::new();

    let response = client
        .post(&format!("{}/subscriptions", &test_app.await.address))
        .header("Content-Type", "application/x-www-form-urlencoded")
        .body(invalid_body)
        .send()
        .await
        .expect("Failed to execute request.");

    assert_eq!(
        400,
        response.status().as_u16(),
        "The API did not fail with 400 Bad Request when the payload was {error_message}."
    );
}
