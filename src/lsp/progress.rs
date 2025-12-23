use serde::Serialize;
use tower_lsp_server::{Client, ls_types as lsp_types};

#[derive(Serialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisStatus {
    Analyzing,
    Finished,
    Error,
}

pub struct ProgressToken {
    client: Option<Client>,
    token: Option<lsp_types::NumberOrString>,
}
impl ProgressToken {
    pub async fn begin(client: Client, message: Option<impl ToString>) -> Self {
        let token = lsp_types::NumberOrString::String(format!("{}", uuid::Uuid::new_v4()));
        client
            .send_request::<lsp_types::request::WorkDoneProgressCreate>(
                lsp_types::WorkDoneProgressCreateParams {
                    token: token.clone(),
                },
            )
            .await
            .ok();

        let value = lsp_types::ProgressParamsValue::WorkDone(lsp_types::WorkDoneProgress::Begin(
            lsp_types::WorkDoneProgressBegin {
                title: "RustOwl".to_owned(),
                cancellable: Some(false),
                message: message.map(|v| v.to_string()),
                percentage: Some(0),
            },
        ));
        client
            .send_notification::<lsp_types::notification::Progress>(lsp_types::ProgressParams {
                token: token.clone(),
                value,
            })
            .await;

        Self {
            client: Some(client),
            token: Some(token),
        }
    }

    pub async fn report(&self, message: Option<impl ToString>, percentage: Option<u32>) {
        if let (Some(client), Some(token)) = (self.client.clone(), self.token.clone()) {
            let value = lsp_types::ProgressParamsValue::WorkDone(
                lsp_types::WorkDoneProgress::Report(lsp_types::WorkDoneProgressReport {
                    cancellable: Some(false),
                    message: message.map(|v| v.to_string()),
                    percentage,
                }),
            );
            client
                .send_notification::<lsp_types::notification::Progress>(lsp_types::ProgressParams {
                    token,
                    value,
                })
                .await;
        }
    }

    pub async fn finish(mut self) {
        let value = lsp_types::ProgressParamsValue::WorkDone(lsp_types::WorkDoneProgress::End(
            lsp_types::WorkDoneProgressEnd { message: None },
        ));
        if let (Some(client), Some(token)) = (self.client.take(), self.token.take()) {
            client
                .send_notification::<lsp_types::notification::Progress>(lsp_types::ProgressParams {
                    token,
                    value,
                })
                .await;
        }
    }
}

impl Drop for ProgressToken {
    fn drop(&mut self) {
        let value = lsp_types::ProgressParamsValue::WorkDone(lsp_types::WorkDoneProgress::End(
            lsp_types::WorkDoneProgressEnd { message: None },
        ));
        if let (Some(client), Some(token)) = (self.client.take(), self.token.take()) {
            tokio::spawn(async move {
                client
                    .send_notification::<lsp_types::notification::Progress>(
                        lsp_types::ProgressParams { token, value },
                    )
                    .await;
            });
        }
    }
}
