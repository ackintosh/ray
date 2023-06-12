use crate::network::ReqId;
use crate::{BehaviourComposer, CombinedKey, NetworkConfig, PeerDB, TARGET_PEERS_COUNT};
use beacon_chain::BeaconChainTypes;
use discv5::Enr;
use libp2p::core::muxing::StreamMuxerBox;
use libp2p::identity::Keypair;
use libp2p::{noise, PeerId, Transport};
use parking_lot::RwLock;
use std::process::exit;
use std::sync::Arc;
use tracing::error;
use types::{ForkContext, MainnetEthSpec};

pub(crate) async fn build_network_transport(
    key_pair: Keypair,
) -> libp2p::core::transport::Boxed<(PeerId, StreamMuxerBox)> {
    let tcp = libp2p::tcp::tokio::Transport::new(libp2p::tcp::Config::default().nodelay(true));
    let transport = libp2p::dns::TokioDnsConfig::system(tcp).unwrap_or_else(|e| {
        error!("Failed to configure DNS: {}", e);
        exit(1);
    });

    // Ref: Why are we using Noise?
    // https://github.com/ethereum/consensus-specs/blob/dev/specs/phase0/p2p-interface.md#why-are-we-using-noise
    let noise_keys = noise::Keypair::<noise::X25519Spec>::new()
        .into_authentic(&key_pair)
        .expect("Signing libp2p-noise static DH keypair failed.");

    transport
        .upgrade(libp2p::core::upgrade::Version::V1)
        .authenticate(noise::NoiseConfig::xx(noise_keys).into_authenticated())
        .multiplex(libp2p::core::upgrade::SelectUpgrade::new(
            libp2p::yamux::YamuxConfig::default(),
            libp2p::mplex::MplexConfig::default(),
        ))
        .timeout(std::time::Duration::from_secs(20))
        .boxed()
}

pub(crate) async fn build_network_behaviour<T: BeaconChainTypes, AppReqId: ReqId>(
    enr: Enr,
    enr_key: CombinedKey,
    network_config: NetworkConfig,
    peer_db: Arc<RwLock<PeerDB>>,
    lh_beacon_chain: Arc<beacon_chain::BeaconChain<T>>,
) -> BehaviourComposer<AppReqId> {
    let mut discovery =
        crate::discovery::behaviour::Behaviour::new(enr, enr_key, &network_config.boot_enr).await;
    // start searching for peers
    discovery.discover_peers();

    let fork_context = Arc::new(ForkContext::new::<MainnetEthSpec>(
        lh_beacon_chain.slot().expect("slot"),
        lh_beacon_chain.genesis_validators_root,
        &lh_beacon_chain.spec,
    ));

    BehaviourComposer::new(
        discovery,
        crate::peer_manager::PeerManager::new(TARGET_PEERS_COUNT, peer_db),
        crate::rpc::behaviour::Behaviour::new(fork_context),
    )
}
