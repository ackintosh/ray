use tracing::info;
use types::{
    BeaconBlock, BeaconState, ChainSpec, EnrForkId, Hash256, MainnetEthSpec, Signature,
    SignedBeaconBlock, Slot,
};

pub(crate) struct BeaconChain {
    pub(crate) chain_spec: ChainSpec,
    // The root of the list of genesis validators, used during syncing.
    pub(crate) genesis_validators_root: Hash256,
    // Stores a "snapshot" of the chain at the time the head-of-the-chain block was received.
    canonical_head: BeaconSnapshot,
}

impl BeaconChain {
    // Starts a new chain from a genesis state.
    pub(crate) fn new(
        chain_spec: ChainSpec,
        mut genesis_state: BeaconState<MainnetEthSpec>,
    ) -> Result<Self, String> {
        let genesis_validators_root = genesis_state.genesis_validators_root();
        let genesis_block = genesis_block(&mut genesis_state, &chain_spec)?;
        let beacon_snapshot = BeaconSnapshot {
            beacon_block: genesis_block,
            beacon_state: genesis_state,
        };

        // TODO: Store the genesis block

        Ok(BeaconChain {
            chain_spec,
            genesis_validators_root,
            canonical_head: beacon_snapshot,
        })
    }

    pub(crate) fn enr_fork_id(&self) -> EnrForkId {
        // NOTE: Fixing to the genesis for now as we don't implement beacon chain yet.
        self.chain_spec.enr_fork_id::<MainnetEthSpec>(
            self.chain_spec.genesis_slot,
            self.genesis_validators_root,
        )
    }

    pub(crate) fn head(&self) -> BeaconSnapshot {
        self.canonical_head.clone()
    }

    pub(crate) fn slot(&self) -> Slot {
        // NOTE: Fixing to the genesis for now as we don't implement beacon chain yet.
        self.chain_spec.genesis_slot
    }

    /// Build a `StatusMessage`
    // ref: https://github.com/sigp/lighthouse/blob/4bf1af4e8520f235de8fe5f94afedf953df5e6a4/beacon_node/network/src/router/processor.rs#L374
    pub(crate) fn create_status_message(&self) -> lighthouse_network::rpc::StatusMessage {
        let enr_fork_id = self.enr_fork_id();
        let head = self.head();
        let finalized_checkpoint = head.beacon_state.finalized_checkpoint();

        lighthouse_network::rpc::StatusMessage {
            fork_digest: enr_fork_id.fork_digest,
            finalized_root: finalized_checkpoint.root,
            finalized_epoch: finalized_checkpoint.epoch,
            head_root: head.beacon_block.canonical_root(),
            head_slot: head.beacon_block.slot(),
        }
    }

    // Determine if the node is relevant to us.
    // ref: https://github.com/sigp/lighthouse/blob/7af57420810772b2a1b0d7d75a0d045c0333093b/beacon_node/network/src/beacon_processor/worker/rpc_methods.rs#L61
    pub(crate) fn is_relevant(
        &self,
        remote_status: &lighthouse_network::rpc::StatusMessage,
    ) -> bool {
        let local_status = self.create_status_message();

        if local_status.fork_digest != remote_status.fork_digest {
            info!(
                "The node is not relevant to us: Incompatible forks. Ours:{} Theirs:{}",
                hex::encode(local_status.fork_digest),
                hex::encode(remote_status.fork_digest)
            );
            return false;
        }

        if remote_status.head_slot > self.slot() {
            info!("The node is not relevant to us: Different system clocks or genesis time");
            return false;
        }

        // NOTE: We can implement more checks to be production-ready.
        // https://github.com/sigp/lighthouse/blob/7af57420810772b2a1b0d7d75a0d045c0333093b/beacon_node/network/src/beacon_processor/worker/rpc_methods.rs#L86-L97

        true
    }
}

// Ref: https://github.com/sigp/lighthouse/blob/99d2c33387477398fc11b55319a064f03ab1a646/beacon_node/beacon_chain/src/builder.rs#L877
fn genesis_block(
    genesis_state: &mut BeaconState<MainnetEthSpec>,
    spec: &ChainSpec,
) -> Result<SignedBeaconBlock<MainnetEthSpec>, String> {
    let mut genesis_block = BeaconBlock::empty(spec);
    *genesis_block.state_root_mut() = genesis_state
        .update_tree_hash_cache()
        .map_err(|e| format!("Failed to hashing genesis state: {:?}", e))?;

    Ok(SignedBeaconBlock::from_block(
        genesis_block,
        Signature::empty(),
    ))
}

// Ref: https://github.com/sigp/lighthouse/blob/b4689e20c6508e58e8245431487e6c645d386ee7/beacon_node/beacon_chain/src/beacon_snapshot.rs#L7
// Represents some block and its associated state. Generally, this will be used for tracking the
// head, justified head and finalized head.
#[derive(Clone, PartialEq, Debug)]
pub(crate) struct BeaconSnapshot {
    pub beacon_block: SignedBeaconBlock<MainnetEthSpec>,
    pub beacon_state: BeaconState<MainnetEthSpec>,
}
