use crate::domain::SubscriberEmail;
use reqwest::Client;
use secrecy::{ExposeSecret, SecretBox};

pub struct EmailClient {
    http_client: Client,
    base_url: String,
    sender: SubscriberEmail,
    authorization_token: String,
}

impl EmailClient {
    pub fn new(
        base_url: &String,
        sender: SubscriberEmail,
        authorization_token: &SecretBox<String>,
        timeout: std::time::Duration,
    ) -> Self {
        let http_client = Client::builder().timeout(timeout).build().unwrap();
        Self {
            http_client,
            base_url: base_url.to_string(),
            sender,
            authorization_token: authorization_token.expose_secret().into(),
        }
    }

    pub async fn create_email(
        &self,
        recipient: SubscriberEmail,
        subject: &str,
        html_content: &str,
        // text_content: &str,
    ) -> Result<(), reqwest::Error> {
        let url = format!("{}/v1.0/me/messages", self.base_url);
        let content = EmailContent {
            content_type: "HTML",
            content: html_content,
        };
        let request_body = SendEmailRequest {
            from: self.sender.as_ref(),
            to: recipient.as_ref(),
            subject,
            content: &content,
        };
        self.http_client
            .post(&url)
            .header("Authorization", &self.authorization_token)
            .json(&request_body)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
    }
}

#[derive(serde::Serialize, Debug)]
#[serde(rename_all = "PascalCase")]
struct SendEmailRequest<'a> {
    from: &'a str,
    to: &'a str,
    subject: &'a str,
    content: &'a EmailContent<'a>,
}

#[derive(serde::Serialize, Debug)]
struct EmailContent<'a> {
    content_type: &'a str,
    content: &'a str,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::SubscriberEmail;
    use claims::{assert_err, assert_ok};
    use fake::{
        faker::{
            internet::en::SafeEmail,
            lorem::en::{Paragraph, Sentence},
        },
        Fake, Faker,
    };
    use secrecy::SecretBox;
    use wiremock::{
        matchers::{any, header, header_exists, method, path},
        Mock, MockServer, Request, ResponseTemplate,
    };

    struct SendEmailBodyMatcher;

    impl wiremock::Match for SendEmailBodyMatcher {
        fn matches(&self, request: &Request) -> bool {
            let result: Result<serde_json::Value, _> = serde_json::from_slice(&request.body);
            if let Ok(body) = result {
                body.get("From").is_some()
                    && body.get("To").is_some()
                    && body.get("Subject").is_some()
                    && body.get("Content").is_some()
            } else {
                false
            }
        }
    }

    fn subject() -> String {
        Sentence(1..2).fake()
    }

    fn content() -> String {
        Paragraph(1..10).fake()
    }

    fn email() -> SubscriberEmail {
        SubscriberEmail::parse(SafeEmail().fake()).unwrap()
    }

    fn email_client(base_url: &String) -> EmailClient {
        EmailClient::new(
            base_url,
            email(),
            &SecretBox::new(Faker.fake()),
            std::time::Duration::from_millis(200),
        )
    }

    #[tokio::test]
    async fn send_email_sends_the_expected_request() {
        let mock_server = MockServer::start().await;
        let email_cient = email_client(&mock_server.uri());

        Mock::given(header_exists("Authorization"))
            .and(header("Content-Type", "application/json"))
            .and(path("/v1.0/me/messages"))
            .and(method("POST"))
            .and(SendEmailBodyMatcher)
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&mock_server)
            .await;

        let _ = email_cient
            .create_email(email(), &subject(), &content())
            .await;
    }

    #[tokio::test]
    async fn send_email_succeeds_if_the_server_returns_200() {
        let mock_server = MockServer::start().await;
        let email_cient = email_client(&mock_server.uri());

        Mock::given(any())
            .respond_with(ResponseTemplate::new(200))
            .expect(1)
            .mount(&mock_server)
            .await;

        let outcome = email_cient
            .create_email(email(), &subject(), &content())
            .await;
        assert_ok!(outcome);
    }

    #[tokio::test]
    async fn send_email_fails_if_the_server_returns_500() {
        let mock_server = MockServer::start().await;
        let email_cient = email_client(&mock_server.uri());

        Mock::given(any())
            .respond_with(ResponseTemplate::new(500))
            .expect(1)
            .mount(&mock_server)
            .await;

        let outcome = email_cient
            .create_email(email(), &subject(), &content())
            .await;
        assert_err!(outcome);
    }

    #[tokio::test]
    async fn send_email_times_out_if_the_server_takes_too_long() {
        let mock_server = MockServer::start().await;
        let email_cient = email_client(&mock_server.uri());

        let response = ResponseTemplate::new(200).set_delay(std::time::Duration::from_secs(180));
        Mock::given(any())
            .respond_with(response)
            .expect(1)
            .mount(&mock_server)
            .await;

        let outcome = email_cient
            .create_email(email(), &subject(), &content())
            .await;
        assert_err!(outcome);
    }
}
