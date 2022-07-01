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

// RPC events sent from RPC behaviour to the behaviour composer
#[derive(Debug)]
#[allow(dead_code)]
pub(crate) enum RpcEvent {
    ReceivedRequest(ReceivedRequest),
    ReceivedResponse(ReceivedResponse),
}

#[derive(Debug)]
pub(crate) struct ReceivedRequest {
    #[allow(dead_code)]
    pub(crate) peer_id: PeerId,
    #[allow(dead_code)]
    pub(crate) request: lighthouse_network::rpc::protocol::InboundRequest<MainnetEthSpec>,
}

#[derive(Debug)]
pub(crate) struct ReceivedResponse {
    #[allow(dead_code)]
    response: lighthouse_network::rpc::methods::RPCResponse<MainnetEthSpec>,
}
