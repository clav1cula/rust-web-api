use rstest::*;
use sqlx::PgPool;
use wiremock::MockServer;
use zero2prod::telemetry::{get_subscriber, init_subscriber};
use zero2prod::{
    configuration::get_configuration,
    startup::{get_connection_pool, Application},
};

pub struct TestApp {
    pub address: String,
    pub db_pool: PgPool,
    pub email_server: MockServer,
    pub port: u16,
}

impl TestApp {
    pub async fn post_subscriptions(&self, body: String) -> reqwest::Response {
        reqwest::Client::new()
            .post(&format!("{}/subscriptions", &self.address))
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .expect("Failed to execute request.")
    }

    pub fn get_confirmation_link(&self, email_request: &wiremock::Request) -> reqwest::Url {
        let body: serde_json::Value = serde_json::from_slice(&email_request.body).unwrap();

        let get_link = |s: &str| {
            let links: Vec<_> = linkify::LinkFinder::new()
                .links(s)
                .filter(|l| *l.kind() == linkify::LinkKind::Url)
                .collect();
            assert_eq!(links.len(), 1);
            let raw_link = links[0].as_str().to_owned();
            let mut confirmation_link = reqwest::Url::parse(&raw_link).unwrap();
            assert_eq!(confirmation_link.host_str().unwrap(), "127.0.0.1");
            confirmation_link.set_port(Some(self.port)).unwrap();
            confirmation_link
        };

        get_link(&body["Content"]["content"].as_str().unwrap())
    }
}

#[fixture]
#[once]
fn set_log() {
    if std::env::var("TEST_LOG").is_ok() {
        let subscriber = get_subscriber("test".into(), "debug".into(), std::io::stdout);
        init_subscriber(subscriber);
    } else {
        let subscriber = get_subscriber("test".into(), "debug".into(), std::io::sink);
        init_subscriber(subscriber);
    }
}

#[fixture]
pub async fn test_app(_set_log: ()) -> TestApp {
    let email_server = MockServer::start().await;
    let mut configuration = get_configuration().expect("Failed to read configuration");
    configuration.application.port = 0;
    configuration.email_client.base_url = email_server.uri();

    let application = Application::build(&configuration)
        .await
        .expect("Failed to build application.");
    let application_port = application.port();
    let address = format!("http://127.0.0.1:{}", &application_port);
    let _ = tokio::spawn(application.run_until_stopped());

    TestApp {
        address,
        db_pool: get_connection_pool(&configuration.database),
        email_server,
        port: application_port,
    }
}
