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
