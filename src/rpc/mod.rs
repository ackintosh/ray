use libp2p::PeerId;
use types::MainnetEthSpec;

pub(crate) mod behaviour;
mod error;
pub(crate) mod handler;
mod message;
mod protocol;

// ////////////////////////////////////////////////////////
// Public events sent by RPC module
// ////////////////////////////////////////////////////////

// RPC events sent from RPC behaviour to the composer
#[derive(Debug)]
#[allow(dead_code)]
pub(crate) enum RpcEvent {
    ReceivedRequest(ReceivedRequest),
}

#[derive(Debug)]
pub(crate) struct ReceivedRequest {
    #[allow(dead_code)]
    peer_id: PeerId,
    #[allow(dead_code)]
    request: lighthouse_network::rpc::protocol::InboundRequest<MainnetEthSpec>,
}
