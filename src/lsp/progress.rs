use serde::Serialize;
use tower_lsp_server::{Client, ls_types};

pub trait ProgressClient: Clone + Send + Sync + 'static {
    fn send_request(&self, token: ls_types::NumberOrString);
    fn send_progress(&self, token: ls_types::NumberOrString, value: ls_types::ProgressParamsValue);
}

impl ProgressClient for Client {
    fn send_request(&self, token: ls_types::NumberOrString) {
        let client = self.clone();
        tokio::spawn(async move {
            client
                .send_request::<ls_types::request::WorkDoneProgressCreate>(
                    ls_types::WorkDoneProgressCreateParams { token },
                )
                .await
                .ok();
        });
    }

    fn send_progress(&self, token: ls_types::NumberOrString, value: ls_types::ProgressParamsValue) {
        let client = self.clone();
        tokio::spawn(async move {
            client
                .send_notification::<ls_types::notification::Progress>(ls_types::ProgressParams {
                    token,
                    value,
                })
                .await;
        });
    }
}

#[derive(Serialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisStatus {
    Analyzing,
    Finished,
    Error,
}

pub struct ProgressToken<C: ProgressClient = Client> {
    client: Option<C>,
    token: Option<ls_types::NumberOrString>,
}

impl ProgressToken<Client> {
    pub async fn begin(client: Client, message: Option<impl ToString>) -> Self {
        ProgressToken::<Client>::begin_with_client(client, message)
    }
}

impl<C: ProgressClient> ProgressToken<C> {
    pub fn begin_with_client(client: C, message: Option<impl ToString>) -> Self {
        let token = ls_types::NumberOrString::String(format!("{}", uuid::Uuid::new_v4()));
        client.send_request(token.clone());

        let value = ls_types::ProgressParamsValue::WorkDone(ls_types::WorkDoneProgress::Begin(
            ls_types::WorkDoneProgressBegin {
                title: "RustOwl".to_owned(),
                cancellable: Some(false),
                message: message.map(|v| v.to_string()),
                percentage: Some(0),
            },
        ));
        client.send_progress(token.clone(), value);

        Self {
            client: Some(client),
            token: Some(token),
        }
    }

    pub fn report(&self, message: Option<impl ToString>, percentage: Option<u32>) {
        if let (Some(client), Some(token)) = (self.client.clone(), self.token.clone()) {
            let value = ls_types::ProgressParamsValue::WorkDone(
                ls_types::WorkDoneProgress::Report(ls_types::WorkDoneProgressReport {
                    cancellable: Some(false),
                    message: message.map(|v| v.to_string()),
                    percentage,
                }),
            );
            client.send_progress(token, value);
        }
    }

    pub fn finish(mut self) {
        let value = ls_types::ProgressParamsValue::WorkDone(ls_types::WorkDoneProgress::End(
            ls_types::WorkDoneProgressEnd { message: None },
        ));
        if let (Some(client), Some(token)) = (self.client.take(), self.token.take()) {
            client.send_progress(token, value);
        }
    }
}

impl<C: ProgressClient> Drop for ProgressToken<C> {
    fn drop(&mut self) {
        let value = ls_types::ProgressParamsValue::WorkDone(ls_types::WorkDoneProgress::End(
            ls_types::WorkDoneProgressEnd { message: None },
        ));
        if let (Some(client), Some(token)) = (self.client.take(), self.token.take()) {
            client.send_progress(token, value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    struct TestClient {
        requests: Arc<Mutex<Vec<ls_types::NumberOrString>>>,
        notifications: Arc<Mutex<Vec<(ls_types::NumberOrString, ls_types::ProgressParamsValue)>>>,
    }

    impl ProgressClient for TestClient {
        fn send_request(&self, token: ls_types::NumberOrString) {
            self.requests.lock().unwrap().push(token);
        }

        fn send_progress(
            &self,
            token: ls_types::NumberOrString,
            value: ls_types::ProgressParamsValue,
        ) {
            self.notifications.lock().unwrap().push((token, value));
        }
    }

    #[test]
    fn progress_token_begin_report_finish_sends_events() {
        let client = TestClient::default();
        let token = ProgressToken::begin_with_client(client.clone(), Some("hello"));
        assert_eq!(client.requests.lock().unwrap().len(), 1);
        assert_eq!(client.notifications.lock().unwrap().len(), 1);

        token.report(Some("step"), Some(50));
        assert_eq!(client.notifications.lock().unwrap().len(), 2);

        token.finish();
        assert_eq!(client.notifications.lock().unwrap().len(), 3);
    }

    #[test]
    fn progress_token_drop_sends_end_once() {
        let client = TestClient::default();
        let token = ProgressToken::begin_with_client(client.clone(), None::<&str>);
        assert_eq!(client.notifications.lock().unwrap().len(), 1);

        drop(token);
        assert_eq!(client.notifications.lock().unwrap().len(), 2);
    }
}
