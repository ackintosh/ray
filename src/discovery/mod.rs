pub(crate) mod behaviour;

use libp2p::PeerId;

// ////////////////////////////////////////////////////////
// Public events sent by Discovery module
// ////////////////////////////////////////////////////////

#[derive(Debug)]
pub(crate) enum DiscoveryEvent {
    // A query has completed. This event contains discovered peer IDs.
    FoundPeers(Vec<PeerId>),
}
