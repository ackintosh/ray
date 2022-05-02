use crate::rpc::handler::Handler;
use crate::rpc::message::{default_finalized_root, Epoch, Root, Slot, Status};
use libp2p::core::connection::ConnectionId;
use libp2p::swarm::{
    ConnectionHandler, DialError, IntoConnectionHandler, NetworkBehaviour, NetworkBehaviourAction,
    NotifyHandler, PollParameters,
};
use libp2p::{Multiaddr, PeerId};
use std::task::{Context, Poll};
use tracing::{info, warn};

pub(crate) struct Behaviour {
    events: Vec<NetworkBehaviourAction<RpcEvent, Handler>>,
}

impl Behaviour {
    pub(crate) fn new() -> Self {
        Behaviour { events: vec![] }
    }

    pub(crate) fn send_status(&mut self, peer_id: PeerId) {
        self.events.push(NetworkBehaviourAction::NotifyHandler {
            peer_id,
            handler: NotifyHandler::Any,
            // TODO: Fill the fields with the real values
            event: RpcEvent::SendStatus(Status {
                fork_digest: [0; 4],
                finalized_root: default_finalized_root(),
                finalized_epoch: Epoch::new(0),
                head_root: Root::from_low_u64_le(0),
                head_slot: Slot::new(0),
            }),
        })
    }
}

// NetworkBehaviour defines "what" bytes to send on the network.
// SEE https://docs.rs/libp2p/0.39.1/libp2p/tutorial/index.html#network-behaviour
impl NetworkBehaviour for Behaviour {
    type ConnectionHandler = Handler;
    type OutEvent = RpcEvent;

    fn new_handler(&mut self) -> Self::ConnectionHandler {
        Handler::new()
    }

    fn addresses_of_peer(&mut self, _peer_id: &PeerId) -> Vec<Multiaddr> {
        info!("addresses_of_peer -> nothing to do because this event is handled by discovery.");
        vec![]
    }

    fn inject_event(
        &mut self,
        peer_id: PeerId,
        _connection: ConnectionId,
        _event: <<Self::ConnectionHandler as IntoConnectionHandler>::Handler as ConnectionHandler>::OutEvent,
    ) {
        info!("inject_event -> {}", peer_id);
    }

    fn inject_dial_failure(
        &mut self,
        peer_id: Option<PeerId>,
        _handler: Self::ConnectionHandler,
        error: &DialError,
    ) {
        warn!(
            "inject_dial_failure: peer_id: {:?}, error: {}",
            peer_id, error
        );
    }

    fn poll(
        &mut self,
        _cx: &mut Context<'_>,
        _params: &mut impl PollParameters,
    ) -> Poll<NetworkBehaviourAction<Self::OutEvent, Self::ConnectionHandler>> {
        info!("poll");

        if !self.events.is_empty() {
            return Poll::Ready(self.events.remove(0));
        }

        Poll::Pending
    }
}

// RPC events sent to handlers
#[derive(Debug)]
pub(crate) enum RpcEvent {
    SendStatus(Status),
}
