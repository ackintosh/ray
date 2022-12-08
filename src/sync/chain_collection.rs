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
        peer_id: PeerId,
        start_epoch: Epoch,
        target_head_root: Hash256,
        target_head_slot: Slot,
    ) {
        let chain_id = crate::sync::syncing_chain::id(&target_head_root, &target_head_slot);

        match self.finalized_chains.entry(chain_id) {
            Entry::Occupied(_) => todo!("Entry::Occupied"),
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

    pub(crate) fn update(&mut self, local_finalized_epoch: Epoch) {
        // TODO: purge outdated chains.

        self.update_finalized_chains(local_finalized_epoch);
    }

    fn update_finalized_chains(&mut self, local_finalized_epoch: Epoch) {
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
        chain.start_syncing(local_finalized_epoch);
    }
}
