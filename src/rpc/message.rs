use primitive_types::H256;

// `compute_fork_digest(current_fork_version, genesis_validators_root)`
type ForkDigest = [u8; 4];

pub(crate) type Root = H256;

#[derive(Debug)]
pub(crate) struct Epoch(u64);

impl Epoch {
    pub(crate) fn new(n: u64) -> Epoch {
        Epoch(n)
    }
}

#[derive(Debug)]
pub(crate) struct Slot(u64);

impl Slot {
    pub(crate) fn new(n: u64) -> Slot {
        Slot(n)
    }
}

// /////////////////////////////////////////////////////////////////////////////////////////////////
// `Status` request/response handshake message.
// https://github.com/ethereum/consensus-specs/blob/dev/specs/phase0/p2p-interface.md#status
//
// lighthouse: https://github.com/sigp/lighthouse/blob/580d2f7873093114bafe5f84f862173eab7d4ff5/beacon_node/lighthouse_network/src/rpc/methods.rs#L61
// /////////////////////////////////////////////////////////////////////////////////////////////////
#[derive(Debug)]
#[allow(dead_code)] // TODO: Remove this
pub(crate) struct Status {
    // The node's ForkDigest.
    pub(crate) fork_digest: ForkDigest,
    // `state.finalized_checkpoint.root` for the state corresponding to the head block.
    // (Note this defaults to Root(b'\x00' * 32) for the genesis finalized checkpoint)
    pub(crate) finalized_root: Root,
    // `state.finalized_checkpoint.epoch` for the state corresponding to the head block.
    pub(crate) finalized_epoch: Epoch,
    // The `hash_tree_root` root of the current head block (`BeaconBlock`).
    pub(crate) head_root: Root,
    // The slot of the block corresponding to the `head_root`.
    pub(crate) head_slot: Slot,
}

// `finalized_root` defaults to Root(b'\x00' * 32) for the genesis finalized checkpoint
pub(crate) fn default_finalized_root() -> Root {
    Root::from_low_u64_le(0)
}
