use crate::peer_db::SyncStatus;
use crate::{BeaconChain, PeerDB};
use libp2p::PeerId;
use parking_lot::RwLock;
use std::cmp::Ordering;
use std::sync::Arc;
use tokio::runtime::Runtime;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tracing::warn;
use types::{Epoch, Hash256, Slot};

#[derive(Debug)]
pub(crate) enum SyncOperation {
    AddPeer(PeerId, SyncInfo),
}

#[derive(Debug)]
pub(crate) struct SyncInfo {
    // Latest finalized root.
    #[allow(dead_code)]
    pub finalized_root: Hash256,
    // Latest finalized epoch.
    #[allow(dead_code)]
    pub finalized_epoch: Epoch,
    // The latest block root.
    #[allow(dead_code)]
    pub head_root: Hash256,
    // The slot associated with the latest block root.
    #[allow(dead_code)]
    pub head_slot: Slot,
}

// The type of peer relative to our current state.
#[derive(Clone)]
enum SyncRelevance {
    // The peer is on our chain and is fully synced with respect to our chain.
    FullySynced,
    // The peer has a greater knowledge of the chain than us that warrants a full sync.
    #[allow(dead_code)]
    Advanced,
    // A peer is behind in the sync and not useful to us for downloading blocks.
    #[allow(dead_code)]
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
    beacon_chain: Arc<RwLock<BeaconChain>>,
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
            .update_sync_status(&peer_id, sync_relevance.clone().into());

        if matches!(sync_relevance, SyncRelevance::Advanced) {
            warn!("TODO: Range sync");
        }
    }

    fn determine_sync_relevance(&self, remote_sync_info: SyncInfo) -> SyncRelevance {
        let local_sync_info: SyncInfo = self.beacon_chain.read().create_status_message().into();

        // NOTE: We can more strict compare.
        // https://github.com/sigp/lighthouse/blob/df40700ddd2dcc3c73859cc3f8e315eab899d87c/beacon_node/network/src/sync/peer_sync_info.rs#L36
        match remote_sync_info
            .finalized_epoch
            .cmp(&local_sync_info.finalized_epoch)
        {
            Ordering::Less => SyncRelevance::Behind,
            Ordering::Equal => SyncRelevance::FullySynced,
            Ordering::Greater => SyncRelevance::Advanced,
        }
    }
}

pub(crate) fn spawn(
    runtime: Arc<Runtime>,
    peer_db: Arc<RwLock<PeerDB>>,
    beacon_chain: Arc<RwLock<BeaconChain>>,
) -> UnboundedSender<SyncOperation> {
    let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();

    let mut sync_manager = SyncManager {
        receiver,
        peer_db,
        beacon_chain,
    };

    runtime.spawn(async move {
        sync_manager.main().await;
    });

    sender
}
