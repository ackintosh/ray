use crate::sync::network_context::SyncNetworkContext;
use libp2p::PeerId;
use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::ops::Sub;
use tracing::{debug, error, info, trace, warn};
use types::{Epoch, EthSpec, Hash256, MainnetEthSpec, Slot};

/// A chain identifier
pub type ChainId = u64;
pub type BatchId = Epoch;

/// Blocks are downloaded in batches from peers. This constant specifies how many epochs worth of
/// blocks per batch are requested.
pub const EPOCHS_PER_BATCH: u64 = 2;

pub(crate) fn id(target_root: &Hash256, target_slot: &Slot) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    (target_root, target_slot).hash(&mut hasher);
    hasher.finish()
}

pub(crate) struct SyncingChain {
    /// A random id used to identify this chain.
    id: ChainId,
    /// The current state of sync.
    state: SyncingState,
    /// The start of the chain segment. Any epoch previous to this one has been validated.
    start_epoch: Epoch,
    /// The target head slot.
    pub(crate) target_head_slot: Slot,
    /// The target head root.
    pub(crate) target_head_root: Hash256,
    /// The peers that agree on the `target_head_slot` and `target_head_root` as a canonical chain
    /// and thus available to download this chain from, as well as the batches we are currently
    /// requesting.
    // peers: Vec<PeerId>,
    peers: HashMap<PeerId, HashSet<BatchId>>,
    /// Starting epoch of the next batch that needs to be downloaded.
    to_be_downloaded: Epoch,
    /// Map of batches undergoing some kind of processing.
    batches: HashMap<Epoch, BatchInfo>,
}

/// A segment of a chain.
struct BatchInfo {
    /// Start slot of the batch.
    start_slot: Slot,
    /// End slot of the batch.
    end_slot: Slot,
}

impl BatchInfo {
    fn new(epoch: Epoch) -> Self {
        // refs: https://github.com/sigp/lighthouse/blob/f4ffa9e0b4acbe3cc3b50f9eeeb6b3d87e58a1a5/beacon_node/network/src/sync/range_sync/batch.rs#L134-L141
        let start_slot = epoch.start_slot(MainnetEthSpec::slots_per_epoch()) + 1;
        let end_slot = start_slot + EPOCHS_PER_BATCH * MainnetEthSpec::slots_per_epoch();
        BatchInfo {
            start_slot,
            end_slot,
        }
    }

    /// Returns a BlocksByRange request associated with the batch.
    fn to_blocks_by_range_request(&self) -> lighthouse_network::rpc::BlocksByRangeRequest {
        lighthouse_network::rpc::BlocksByRangeRequest {
            start_slot: self.start_slot.into(),
            count: self.end_slot.sub(self.start_slot).into(),
        }
    }
}

#[derive(Debug)]
enum SyncingState {
    /// The chain is not being synced.
    Stopped,
    /// The chain is undergoing syncing.
    Syncing,
}

impl SyncingChain {
    pub(crate) fn new(
        start_epoch: Epoch,
        target_head_slot: Slot,
        target_head_root: Hash256,
        peer_id: PeerId,
    ) -> Self {
        let id = id(&target_head_root, &target_head_slot);
        let mut peers = HashMap::new();
        peers.insert(peer_id, HashSet::new());

        SyncingChain {
            id,
            state: SyncingState::Stopped,
            start_epoch,
            target_head_slot,
            target_head_root,
            peers,
            to_be_downloaded: start_epoch,
            batches: HashMap::new(),
        }
    }

    pub(crate) fn available_peers(&self) -> usize {
        self.peers.len()
    }

    /// Add a peer to the chain.
    ///
    /// If the chain is active, this starts requesting batches from this peer.
    pub(crate) fn add_peer(&mut self, network: &mut SyncNetworkContext, peer_id: PeerId) {
        if self.peers.entry(peer_id).or_default().is_empty() {
            self.request_batches(network);
        }
    }

    pub(crate) fn start_syncing(
        &mut self,
        network_context: &mut SyncNetworkContext,
        local_finalized_epoch: Epoch,
    ) {
        debug!("start_syncing: chain_id: {}", self.id);

        // NOTE: Ideally we should align the epochs
        // https://github.com/sigp/lighthouse/blob/8c69d57c2ce0d5f1a3cd44c215b2d52844043150/beacon_node/network/src/sync/range_sync/chain.rs#L779

        self.advance_chain(local_finalized_epoch);

        self.state = SyncingState::Syncing;
        self.request_batches(network_context);
    }

    fn advance_chain(&mut self, local_finalized_epoch: Epoch) {
        // make sure this epoch produces an advancement
        if local_finalized_epoch <= self.start_epoch {
            debug!("advance_chain: local_finalized_epoch is before or equal than the start_epoch of the syncing chain.");
            return;
        }

        // TODO: some batch processing should be implemented
        // https://github.com/sigp/lighthouse/blob/8c69d57c2ce0d5f1a3cd44c215b2d52844043150/beacon_node/network/src/sync/range_sync/chain.rs#L624

        let old_start_epoch = self.start_epoch;
        self.start_epoch = local_finalized_epoch;
        info!(
            "SyncingChain has been advanced from {old_start_epoch} to {}",
            self.start_epoch
        );
    }

    fn request_batches(&mut self, network_context: &mut SyncNetworkContext) {
        if !matches!(self.state, SyncingState::Syncing) {
            warn!("sync state is not Syncing: {:?}", self.state);
            return;
        }

        // NOTE: The peer pool should be shuffled before sending request for load balancing.
        // https://github.com/sigp/lighthouse/blob/8c69d57c2ce0d5f1a3cd44c215b2d52844043150/beacon_node/network/src/sync/range_sync/chain.rs#L985

        for (peer_id, _batches) in self.peers.clone().iter() {
            if let Some(epoch) = self.next_batch() {
                self.send_batch(network_context, peer_id, epoch);
            } else {
                // No more batches, simply stop
                return;
            }
        }
    }

    /// Creates the next required batch from the chain. If there are no more batches required,
    /// `None` is returned.
    fn next_batch(&mut self) -> Option<Epoch> {
        // don't request batches beyond the target head slot
        if self
            .to_be_downloaded
            .start_slot(MainnetEthSpec::slots_per_epoch())
            >= self.target_head_slot
        {
            return None;
        }

        // NOTE: making buffer size limit would be better.
        // https://github.com/sigp/lighthouse/blob/8c69d57c2ce0d5f1a3cd44c215b2d52844043150/beacon_node/network/src/sync/range_sync/chain.rs#L1037

        let epoch = self.to_be_downloaded;
        match self.batches.entry(epoch) {
            Entry::Occupied(_) => {
                // this batch doesn't need downloading, let this same function decide the next batch
                self.to_be_downloaded += EPOCHS_PER_BATCH;
                self.next_batch()
            }
            Entry::Vacant(entry) => {
                entry.insert(BatchInfo::new(epoch));
                self.to_be_downloaded += EPOCHS_PER_BATCH;
                Some(epoch)
            }
        }
    }

    /// Requests the batch assigned to the given epoch (batch id) from a given peer.
    fn send_batch(
        &mut self,
        network_context: &mut SyncNetworkContext,
        peer_id: &PeerId,
        epoch: Epoch,
    ) {
        trace!("[{peer_id}] [SyncingChain::send_batch] epoch(batch_id):{epoch}");

        let batch_info = match self.batches.get_mut(&epoch) {
            Some(batch_info) => batch_info,
            None => {
                warn!("[{peer_id}] [SyncingChain::send_batch] BatchInfo not found. epoch:{epoch}");
                return;
            }
        };

        let request = batch_info.to_blocks_by_range_request();
        match network_context.blocks_by_range_request(peer_id, request) {
            Ok(_request_id) => {
                // TODO: store the request_id in self.peers
                // https://github.com/ackintosh/lighthouse/blob/8c69d57c2ce0d5f1a3cd44c215b2d52844043150/beacon_node/network/src/sync/range_sync/chain.rs#L902
            }
            Err(e) => {
                error!("[{peer_id}] [SyncingChain::send_batch] Failed to send `BlocksByRange` request. error:{e}")
                // TODO: error handling
                // https://github.com/ackintosh/lighthouse/blob/8c69d57c2ce0d5f1a3cd44c215b2d52844043150/beacon_node/network/src/sync/range_sync/chain.rs#L929
            }
        }
    }
}
