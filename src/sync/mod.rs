use crate::PeerDB;
use libp2p::PeerId;
use parking_lot::RwLock;
use std::sync::Arc;
use tokio::runtime::Runtime;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use types::{Epoch, Hash256, Slot};

#[derive(Debug)]
pub(crate) enum SyncOperation {
    AddPeer(PeerId, SyncInfo),
}

#[derive(Debug)]
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
    peer_db: Arc<RwLock<PeerDB>>,
    receiver: UnboundedReceiver<SyncOperation>,
}

impl SyncManager {
    async fn main(&mut self) {
        loop {
            // Process inbound messages
            if let Some(operation) = self.receiver.recv().await {
                match operation {
                    SyncOperation::AddPeer(peer_id, _sync_info) => {
                        self.add_peer(peer_id);
                    }
                }
            }
        }
    }

    fn add_peer(&mut self, _peer_id: PeerId) {
        todo!();
    }
}

pub(crate) fn spawn(
    runtime: Arc<Runtime>,
    peer_db: Arc<RwLock<PeerDB>>,
) -> UnboundedSender<SyncOperation> {
    let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();

    let mut sync_manager = SyncManager { receiver, peer_db };

    runtime.spawn(async move {
        sync_manager.main().await;
    });

    sender
}
