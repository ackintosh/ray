use std::sync::Arc;
use tokio::runtime::Runtime;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

#[derive(Debug)]
pub(crate) enum SyncOperation {
    AddPeer,
}

pub(crate) struct SyncManager {
    receiver: UnboundedReceiver<SyncOperation>,
}

impl SyncManager {
    async fn main(&mut self) {
        loop {
            if let Some(operation) = self.receiver.recv().await {
                println!("{:?}", operation);
            }
        }
    }
}

pub(crate) fn spawn(runtime: Arc<Runtime>) -> UnboundedSender<SyncOperation> {
    let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();

    let mut sync_manager = SyncManager { receiver };

    runtime.spawn(async move {
        sync_manager.main().await;
    });

    sender
}
