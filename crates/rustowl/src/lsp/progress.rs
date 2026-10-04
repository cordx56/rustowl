use serde::Serialize;
use std::sync::atomic::{AtomicI32, Ordering};
use tower_lsp_server::{Bounded, Client, NotCancellable, OngoingProgress, gen_lsp_types};

#[derive(Serialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisStatus {
    Analyzing,
    Finished,
    Error,
}

/// Work-done progress tokens must be unique per operation. Analyses are driven
/// from a single server process, so a counter is enough to tell them apart.
static NEXT_TOKEN: AtomicI32 = AtomicI32::new(0);

type Handle = OngoingProgress<Bounded, NotCancellable>;

pub struct ProgressToken(Option<Handle>);

impl ProgressToken {
    pub async fn begin(client: Client) -> Self {
        let token = NEXT_TOKEN.fetch_add(1, Ordering::Relaxed);
        let handle = client
            .progress(gen_lsp_types::ProgressToken::Int(token), "RustOwl")
            .with_percentage(0)
            .begin()
            .await;
        Self(Some(handle))
    }

    pub async fn report(&self, message: impl Into<String>, percentage: u32) {
        if let Some(handle) = &self.0 {
            handle.report_with_message(message, percentage).await;
        }
    }

    pub async fn finish(mut self) {
        if let Some(handle) = self.0.take() {
            handle.finish().await;
        }
    }
}

impl Drop for ProgressToken {
    /// Finishes progress when an analysis is cancelled before calling `finish`.
    fn drop(&mut self) {
        if let Some(handle) = self.0.take() {
            tokio::spawn(async move {
                handle.finish().await;
            });
        }
    }
}
