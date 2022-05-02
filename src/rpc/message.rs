use crate::types::{Epoch, ForkDigest, Root, Slot};

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
