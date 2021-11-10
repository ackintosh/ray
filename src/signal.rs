use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use tokio::runtime::Runtime;
use tokio::signal::unix::{signal, Signal, SignalKind};
use tracing::error;

pub(crate) fn block_until_shutdown_requested(
    runtime: Arc<Runtime>,
) -> (Option<&'static str>, usize, Vec<SignalFuture>) {
    runtime.block_on(async {
        let mut handles = vec![];

        match signal(SignalKind::terminate()) {
            Ok(terminate_stream) => {
                let terminate = SignalFuture::new(terminate_stream, "Received SIGTERM");
                handles.push(terminate);
            }
            Err(e) => error!("Could not register SIGTERM handler: {}", e),
        }

        match signal(SignalKind::interrupt()) {
            Ok(interrupt_stream) => {
                let interrupt = SignalFuture::new(interrupt_stream, "Received SIGINT");
                handles.push(interrupt);
            }
            Err(e) => error!("Could not register SIGINT handler: {}", e),
        }

        futures::future::select_all(handles.into_iter()).await
    })
}

// SEE: https://github.com/sigp/lighthouse/blob/d9910f96c5f71881b88eec15253b31890bcd28d2/lighthouse/environment/src/lib.rs#L492
#[cfg(target_family = "unix")]
pub(crate) struct SignalFuture {
    signal: Signal,
    message: &'static str,
}

#[cfg(target_family = "unix")]
impl SignalFuture {
    pub fn new(signal: Signal, message: &'static str) -> SignalFuture {
        SignalFuture { signal, message }
    }
}

#[cfg(target_family = "unix")]
impl Future for SignalFuture {
    type Output = Option<&'static str>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.signal.poll_recv(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Some(_)) => Poll::Ready(Some(self.message)),
            Poll::Ready(None) => Poll::Ready(None),
        }
    }
}
