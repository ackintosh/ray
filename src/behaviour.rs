use libp2p::NetworkBehaviour;

// core behaviour that combines the sub-behaviours.
#[derive(NetworkBehaviour)]
pub(crate) struct Behaviour {
    /* Sub-Behaviours */
    rpc: crate::rpc::behavior::Behaviour,
}

impl Behaviour {
    pub(crate) fn new(rpc: crate::rpc::behavior::Behaviour) -> Self {
        Self {
            rpc,
        }
    }
}