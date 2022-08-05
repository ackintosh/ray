use crate::peer_db::SyncStatus;
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
    // Latest finalized root.
    pub finalized_root: Hash256,
    // Latest finalized epoch.
    pub finalized_epoch: Epoch,
    // The latest block root.
    pub head_root: Hash256,
    // The slot associated with the latest block root.
    pub head_slot: Slot,
}

// The type of peer relative to our current state.
enum SyncRelevance {
    // The peer is on our chain and is fully synced with respect to our chain.
    FullySynced,
    // The peer has a greater knowledge of the chain than us that warrants a full sync.
    Advanced,
    // A peer is behind in the sync and not useful to us for downloading blocks.
    Behind,
}

impl From<SyncRelevance> for SyncStatus {
    fn from(relevance: SyncRelevance) -> Self {
        match relevance {
            SyncRelevance::FullySynced => SyncStatus::Synced,
            SyncRelevance::Advanced => SyncStatus::Advanced,
            SyncRelevance::Behind => SyncStatus::Behind,
        }
    }
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
                    SyncOperation::AddPeer(peer_id, sync_info) => {
                        self.add_peer(peer_id, sync_info);
                    }
                }
            }
        }
    }

    fn add_peer(&mut self, peer_id: PeerId, sync_info: SyncInfo) {
        let sync_relevance = self.determine_sync_relevance(sync_info);
        self.peer_db
            .write()
            .update_sync_status(&peer_id, sync_relevance.into());
    }

    fn determine_sync_relevance(&self, _sync_info: SyncInfo) -> SyncRelevance {
        // TODO
        SyncRelevance::FullySynced
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
