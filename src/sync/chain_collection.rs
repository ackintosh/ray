use crate::sync::network_context::SyncNetworkContext;
use crate::sync::syncing_chain::{ChainId, SyncingChain};
use libp2p::PeerId;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use tracing::{info, warn};
use types::{Epoch, Hash256, Slot};

pub(crate) struct ChainCollection {
    /// The current sync state of the process.
    state: RangeSyncState,
    /// The set of finalized chains being synced.
    finalized_chains: HashMap<ChainId, SyncingChain>,
}

enum RangeSyncState {
    /// There are no finalized chains and no long range sync is in progress.
    Idle,
    /// A finalized chain is being synced.
    Syncing(ChainId),
}

impl ChainCollection {
    pub(crate) fn new() -> Self {
        ChainCollection {
            state: RangeSyncState::Idle,
            finalized_chains: HashMap::new(),
        }
    }

    pub(crate) fn add_peer_or_create_chain(
        &mut self,
        network_context: &mut SyncNetworkContext,
        peer_id: PeerId,
        start_epoch: Epoch,
        target_head_root: Hash256,
        target_head_slot: Slot,
    ) {
        let chain_id = crate::sync::syncing_chain::id(&target_head_root, &target_head_slot);

        match self.finalized_chains.entry(chain_id) {
            Entry::Occupied(mut entry) => {
                info!(chain_id = %chain_id, "[{peer_id}] Adding peer to known chain.");
                let chain = entry.get_mut();
                assert_eq!(chain.target_head_root, target_head_root);
                assert_eq!(chain.target_head_slot, target_head_slot);
                chain.add_peer(network_context, peer_id);
            }
            Entry::Vacant(entry) => {
                info!("[{peer_id}] A new finalized chain is added to sync. chain_id: {chain_id}");

                entry.insert(SyncingChain::new(
                    start_epoch,
                    target_head_slot,
                    target_head_root,
                    peer_id,
                ));
            }
        }
    }

    pub(crate) fn update(
        &mut self,
        network_context: &mut SyncNetworkContext,
        local_finalized_epoch: Epoch,
    ) {
        // TODO: purge outdated chains.

        self.update_finalized_chains(network_context, local_finalized_epoch);
    }

    fn update_finalized_chains(
        &mut self,
        network_context: &mut SyncNetworkContext,
        local_finalized_epoch: Epoch,
    ) {
        let new_syncing_chain_id = {
            let chain_id = self
                .finalized_chains
                .iter()
                .max_by_key(|(_id, chain)| chain.available_peers())
                .map(|(id, _chain)| *id);

            if let Some(id) = chain_id {
                id
            } else {
                warn!("No finalized chains found.");
                return;
            }
        };

        let chain = self
            .finalized_chains
            .get_mut(&new_syncing_chain_id)
            .expect("Syncing chain exists.");

        match self.state {
            RangeSyncState::Idle => {
                info!("Syncing new finalized chain. chain_id: {new_syncing_chain_id}");
            }
            RangeSyncState::Syncing(id) => {
                if new_syncing_chain_id != id {
                    info!("Switching finalized chains from {id} to {new_syncing_chain_id}");
                }
            }
        }

        self.state = RangeSyncState::Syncing(new_syncing_chain_id);
        chain.start_syncing(network_context, local_finalized_epoch);
    }
}
