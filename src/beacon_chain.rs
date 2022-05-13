use types::ChainSpec;

pub(crate) struct BeaconChain {
    chain_spec: ChainSpec,
}

impl BeaconChain {
    pub(crate) fn new(chain_spec: ChainSpec) -> Self {
        BeaconChain { chain_spec }
    }
}
