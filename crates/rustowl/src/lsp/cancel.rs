//! A cancellation signal that never blocks.
//!
//! `tokio_util`'s token keeps its children behind a lock; this one keeps a
//! flag and a notification, so no thread ever waits for another to progress.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::Notify;

#[derive(Clone, Default)]
pub struct Cancel {
    flag: Arc<AtomicBool>,
    notify: Arc<Notify>,
}

impl Cancel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.flag.store(true, Ordering::Release);
        self.notify.notify_waiters();
    }

    pub fn is_cancelled(&self) -> bool {
        self.flag.load(Ordering::Acquire)
    }

    pub async fn cancelled(&self) {
        // The wait is registered before the flag is read: a cancel that
        // lands in between must not be missed.
        let notified = self.notify.notified();
        tokio::pin!(notified);
        notified.as_mut().enable();
        if self.is_cancelled() {
            return;
        }
        notified.await;
    }
}

#[cfg(all(test, not(miri)))]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_cancelled_signal_wakes_its_waiters() {
        let cancel = Cancel::new();
        assert!(!cancel.is_cancelled());

        let waiter = tokio::spawn({
            let cancel = cancel.clone();
            async move {
                cancel.cancelled().await;
            }
        });

        cancel.cancel();
        waiter.await.expect("the waiter must wake");
        assert!(cancel.is_cancelled());
    }

    #[tokio::test]
    async fn a_signal_cancelled_before_the_wait_is_seen() {
        let cancel = Cancel::new();
        cancel.cancel();

        // The wait must return at once: the cancel happened before anyone registered.
        cancel.cancelled().await;
    }
}
