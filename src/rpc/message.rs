// `Status` request/response handshake message.
// https://github.com/ethereum/consensus-specs/blob/dev/specs/phase0/p2p-interface.md#status
// lighthouse: https://github.com/sigp/lighthouse/blob/580d2f7873093114bafe5f84f862173eab7d4ff5/beacon_node/lighthouse_network/src/rpc/methods.rs#L61
#[derive(Debug)]
#[allow(dead_code)] // TODO: Remove this
pub(crate) struct Status {
    pub(crate) fork_digest: u64,
    pub(crate) finalized_root: u64,
    pub(crate) finalized_epoch: u64,
    pub(crate) head_root: u64,
    pub(crate) head_slot: u64,
}
