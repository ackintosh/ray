use libp2p::NetworkBehaviour;

// The core behaviour that combines the sub-behaviours.
#[derive(NetworkBehaviour)]
pub(crate) struct BehaviourComposer {
    /* Sub-Behaviours */
    discovery: crate::discovery::behaviour::Behaviour,
    rpc: crate::rpc::behaviour::Behaviour,
}

impl BehaviourComposer {
    pub(crate) fn new(
        discovery: crate::discovery::behaviour::Behaviour,
        rpc: crate::rpc::behaviour::Behaviour,
    ) -> Self {
        Self {
            discovery,
            rpc,
        }
    }
}