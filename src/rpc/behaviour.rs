use crate::rpc::handler::Handler;
use crate::rpc::message::Status;
use crate::types::{Epoch, ForkDigest, Root, Slot};
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

    // https://github.com/ethereum/consensus-specs/blob/dev/specs/phase0/p2p-interface.md#status
    pub(crate) fn send_status(
        &mut self,
        peer_id: PeerId,
        fork_digest: ForkDigest,
        finalized_root: Root,
        finalized_epoch: Epoch,
        head_root: Root,
        head_slot: Slot,
    ) {
        self.events.push(NetworkBehaviourAction::NotifyHandler {
            peer_id,
            handler: NotifyHandler::Any,
            event: RpcEvent::SendStatus(Status {
                fork_digest,
                finalized_root,
                finalized_epoch,
                head_root,
                head_slot,
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
