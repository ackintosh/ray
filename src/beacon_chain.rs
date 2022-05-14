use types::{ChainSpec, EnrForkId, Hash256, MainnetEthSpec};

pub(crate) struct BeaconChain {
    chain_spec: ChainSpec,
    // The root of the list of genesis validators, used during syncing.
    genesis_validators_root: Hash256,
}

impl BeaconChain {
    pub(crate) fn new(chain_spec: ChainSpec, genesis_validators_root: Hash256) -> Self {
        BeaconChain {
            chain_spec,
            genesis_validators_root,
        }
    }

    pub(crate) fn enr_fork_id(&self) -> EnrForkId {
        // NOTE: Fixing to the genesis for now as we don't implement beacon chain yet.
        self.chain_spec
            .enr_fork_id::<MainnetEthSpec>(self.chain_spec.genesis_slot, self.genesis_validators_root)
    }
}
