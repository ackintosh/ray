use beacon_chain::{BeaconChain, BeaconChainTypes};
use lighthouse_network::rpc::StatusMessage;
use types::{EthSpec, Hash256, MainnetEthSpec};

// refs: https://github.com/sigp/lighthouse/blob/be4e261e7433e02983648f7d7d8f21f74d3fa9d8/beacon_node/network/src/status.rs#L20
pub(crate) fn status_message<T: BeaconChainTypes>(chain: &BeaconChain<T>) -> StatusMessage {
    let fork_digest = chain.enr_fork_id().fork_digest;
    let cached_head = chain.canonical_head.cached_head();
    let mut finalized_checkpoint = cached_head.finalized_checkpoint();

    // Alias the genesis checkpoint root to `0x00`.
    let spec = &chain.spec;
    let genesis_epoch = spec.genesis_slot.epoch(MainnetEthSpec::slots_per_epoch());
    if finalized_checkpoint.epoch == genesis_epoch {
        finalized_checkpoint.root = Hash256::zero();
    }

    StatusMessage {
        fork_digest,
        finalized_root: finalized_checkpoint.root,
        finalized_epoch: finalized_checkpoint.epoch,
        head_root: cached_head.head_block_root(),
        head_slot: cached_head.head_slot(),
    }
}
