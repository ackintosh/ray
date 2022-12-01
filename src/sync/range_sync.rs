use crate::sync::chain_collection::ChainCollection;
use crate::sync::SyncInfo;
use beacon_node::beacon_chain::BeaconChainTypes;
use libp2p::PeerId;
use std::sync::Arc;
use tracing::warn;

pub(crate) struct RangeSync<T: BeaconChainTypes> {
    /// The beacon chain for processing.
    lh_beacon_chain: Arc<beacon_node::beacon_chain::BeaconChain<T>>,
    /// A collection of chains that need to be downloaded. This stores any head or finalized chains
    /// that need to be downloaded.
    chains: ChainCollection,
}

impl<T> RangeSync<T>
where
    T: BeaconChainTypes,
{
    pub(crate) fn new(lh_beacon_chain: Arc<beacon_node::beacon_chain::BeaconChain<T>>) -> Self {
        RangeSync {
            lh_beacon_chain,
            chains: ChainCollection::new(),
        }
    }

    pub(crate) fn add_peer(
        &mut self,
        peer_id: PeerId,
        local_sync_info: &SyncInfo,
        remote_sync_info: &SyncInfo,
    ) {
        let is_block_known = false; // TODO

        // determine which kind of sync to perform and set up the chains
        match RangeSyncType::new(local_sync_info, remote_sync_info, is_block_known) {
            RangeSyncType::Finalized => {
                self.chains.add_peer_or_create_chain(
                    peer_id,
                    local_sync_info.finalized_epoch.clone(),
                    remote_sync_info.finalized_root.clone(),
                    remote_sync_info.head_slot.clone(),
                );
            }
            RangeSyncType::Head => {
                warn!("[{peer_id}] RangeSyncType::Head is not supported.");
            }
        }

        self.chains.update();
    }
}

/// The type of Range sync that should be done relative to our current state.
pub(crate) enum RangeSyncType {
    /// A finalized chain sync should be started with this peer.
    Finalized,
    /// A head chain sync should be started with this peer.
    Head,
}

impl RangeSyncType {
    // https://github.com/sigp/lighthouse/blob/31386277c3bd7966fcf97789ef2b4834e5452af9/beacon_node/network/src/sync/range_sync/sync_type.rs#L20
    pub(crate) fn new(
        local_sync_info: &SyncInfo,
        remote_sync_info: &SyncInfo,
        is_block_known: bool,
    ) -> Self {
        // Check for finalized chain sync
        //
        // The condition is:
        // -  The remotes finalized epoch is greater than our current finalized epoch and we have
        //    not seen the finalized hash before.
        if remote_sync_info.finalized_root > local_sync_info.finalized_root && !is_block_known {
            RangeSyncType::Finalized
        } else {
            RangeSyncType::Head
        }
    }
}
