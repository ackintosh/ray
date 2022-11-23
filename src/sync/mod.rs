mod range_sync;
mod syncing_chain;

use crate::peer_db::SyncStatus;
use crate::sync::range_sync::RangeSync;
use crate::PeerDB;
use libp2p::PeerId;
use parking_lot::RwLock;
use std::cmp::Ordering;
use std::sync::Arc;
use beacon_node::beacon_chain::BeaconChainTypes;
use tokio::runtime::Runtime;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tracing::warn;
use types::{Epoch, Hash256, Slot};
use crate::rpc::status::status_message;

#[derive(Debug)]
/// A message that can be sent to the sync manager thread.
pub(crate) enum SyncOperation {
    /// A useful peer has been discovered.
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

pub(crate) struct SyncManager<T: BeaconChainTypes> {
    peer_db: Arc<RwLock<PeerDB>>,
    lh_beacon_chain: Arc<beacon_node::beacon_chain::BeaconChain<T>>,
    receiver: UnboundedReceiver<SyncOperation>,
    range_sync: RangeSync<T>,
}

impl<T> SyncManager<T>
where
    T: BeaconChainTypes
{
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

    /// A peer has connected which has blocks that are unknown to us.
    fn add_peer(&mut self, peer_id: PeerId, remote_sync_info: SyncInfo) {
        let local_sync_info: SyncInfo = status_message(&self.lh_beacon_chain).into();
        let sync_relevance = self.determine_sync_relevance(&local_sync_info, &remote_sync_info);

        // update the state of the peer.
        self.peer_db
            .write()
            .update_sync_status(&peer_id, sync_relevance.clone().into());

        if matches!(sync_relevance, SyncRelevance::Advanced) {
            self.range_sync
                .add_peer(peer_id, &local_sync_info, &remote_sync_info);
        }
    }

    fn determine_sync_relevance(
        &self,
        local_sync_info: &SyncInfo,
        remote_sync_info: &SyncInfo,
    ) -> SyncRelevance {
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

pub(crate) fn spawn<T: BeaconChainTypes>(
    runtime: Arc<Runtime>,
    peer_db: Arc<RwLock<PeerDB>>,
    lh_beacon_chain: Arc<beacon_node::beacon_chain::BeaconChain<T>>,
) -> UnboundedSender<SyncOperation> {
    let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();

    let mut sync_manager = SyncManager {
        receiver,
        peer_db,
        lh_beacon_chain: lh_beacon_chain.clone(),
        range_sync: RangeSync::new(lh_beacon_chain.clone()),
    };

    runtime.spawn(async move {
        sync_manager.main().await;
    });

    sender
}
