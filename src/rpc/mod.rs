use crate::rpc::handler::SubstreamId;
use libp2p::core::connection::ConnectionId;
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
    pub(crate) peer_id: PeerId,
    pub(crate) connection_id: ConnectionId,
    pub(crate) substream_id: SubstreamId,
    #[allow(dead_code)]
    pub(crate) request: lighthouse_network::rpc::protocol::InboundRequest<MainnetEthSpec>,
}

#[derive(Debug)]
pub(crate) struct ReceivedResponse {
    pub(crate) peer_id: PeerId,
    pub(crate) response: lighthouse_network::rpc::methods::RPCResponse<MainnetEthSpec>,
}
