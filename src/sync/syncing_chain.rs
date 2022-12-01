use libp2p::PeerId;
use std::hash::{Hash, Hasher};
use std::ptr::hash;
use types::{Epoch, Hash256, Slot};

/// A chain identifier
pub type ChainId = u64;

pub(crate) fn id(target_root: &Hash256, target_slot: &Slot) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    (target_root, target_slot).hash(&mut hasher);
    hasher.finish()
}

pub(crate) struct SyncingChain {
    /// A random id used to identify this chain.
    id: ChainId,
    /// The start of the chain segment. Any epoch previous to this one has been validated.
    start_epoch: Epoch,
    /// The target head slot.
    target_head_slot: Slot,
    /// The target head root.
    target_head_root: Hash256,
    /// The peers that agree on the `target_head_slot` and `target_head_root` as a canonical chain
    /// and thus available to download this chain from, as well as the batches we are currently
    /// requesting.
    peers: Vec<PeerId>,
}

impl SyncingChain {
    pub(crate) fn new(
        start_epoch: Epoch,
        target_head_slot: Slot,
        target_head_root: Hash256,
        peer_id: PeerId,
    ) -> Self {
        let id = id(&target_head_root, &target_head_slot);

        SyncingChain {
            id,
            start_epoch,
            target_head_slot,
            target_head_root,
            peers: vec![peer_id],
        }
    }

    pub(crate) fn available_peers(&self) -> usize {
        self.peers.len()
    }

    pub(crate) fn start_syncing(&mut self) {
        // NOTE: Ideally we should align the epochs
        // https://github.com/sigp/lighthouse/blob/8c69d57c2ce0d5f1a3cd44c215b2d52844043150/beacon_node/network/src/sync/range_sync/chain.rs#L779

    }
}
