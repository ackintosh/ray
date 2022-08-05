use std::sync::Arc;
use libp2p::PeerId;
use tokio::runtime::Runtime;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use types::{Epoch, Hash256, Slot};

#[derive(Debug)]
pub(crate) enum SyncOperation {
    AddPeer(PeerId, SyncInfo),
}

pub(crate) struct SyncInfo {
    /// Latest finalized root.
    pub finalized_root: Hash256,
    /// Latest finalized epoch.
    pub finalized_epoch: Epoch,
    /// The latest block root.
    pub head_root: Hash256,
    /// The slot associated with the latest block root.
    pub head_slot: Slot,
}

impl From<lighthouse_network::rpc::StatusMessage> for SyncInfo {
    fn from(status: lighthouse_network::rpc::StatusMessage) -> Self {
        Self {
            finalized_root: status.finalized_root,
            finalized_epoch: status.finalized_epoch,
            head_root: status.head_root,
            head_slot: status.head_slot,
        }
    }
}

pub(crate) struct SyncManager {
    receiver: UnboundedReceiver<SyncOperation>,
}

impl SyncManager {
    async fn main(&mut self) {
        loop {
            // Process inbound messages
            if let Some(operation) = self.receiver.recv().await {
                match operation {
                    SyncOperation::AddPeer(_peer_id, _sync_info) => {
                        todo!();
                    }
                }
            }
        }
    }

    fn add_peer(&mut self, peer_id: PeerId) {

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
