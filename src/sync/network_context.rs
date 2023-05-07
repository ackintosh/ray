use crate::network::{ApplicationRequestId, NetworkMessage};
use crate::sync::SyncRequestId::RangeSync;
use libp2p::PeerId;
use tokio::sync::mpsc::UnboundedSender;
use tracing::trace;

/// Wraps a Network channel to employ various RPC related network functionality for the Sync manager.
/// This includes management of a global RPC request Id.
pub(crate) struct SyncNetworkContext {
    /// A sequential ID for all RPC requests.
    request_id: u32,
    /// The network channel to relay messages to the Network service.
    network_send: UnboundedSender<NetworkMessage>,
}

impl SyncNetworkContext {
    pub(crate) fn new(network_send: UnboundedSender<NetworkMessage>) -> SyncNetworkContext {
        SyncNetworkContext {
            request_id: 0,
            network_send,
        }
    }

    pub(crate) fn blocks_by_range_request(
        &mut self,
        peer_id: &PeerId,
        request: lighthouse_network::rpc::BlocksByRangeRequest,
    ) -> Result<u32, String> {
        trace!("[{peer_id}] [SyncNetworkContext::blocks_by_range_request] Sending `BlocksByRange` request to the network component. request: {request:?}");

        let request = lighthouse_network::service::api_types::Request::BlocksByRange(request);
        let id = self.next_id();
        let request_id = ApplicationRequestId::Sync(RangeSync { id });
        // network::service::RequestId::Sync(network::sync::manager::RequestId::RangeSync { id });

        self.network_send
            .send(NetworkMessage::SendRequest {
                peer_id: peer_id.clone(),
                request,
                request_id,
            })
            .map_err(|e| format!("Failed to send NetworkMessage: {e}"))?;

        Ok(id)
    }

    fn next_id(&mut self) -> u32 {
        let id = self.request_id;
        self.request_id += 1;
        id
    }
}
