use crate::{BeaconChain, BehaviourComposer, CombinedKey, NetworkConfig, TARGET_PEERS_COUNT};
use discv5::Enr;
use libp2p::core::muxing::StreamMuxerBox;
use libp2p::identity::Keypair;
use libp2p::{noise, PeerId, Transport};
use std::process::exit;
use std::sync::Arc;
use tracing::error;
use types::{ForkContext, MainnetEthSpec};

pub(crate) async fn build_network_transport(
    key_pair: Keypair,
) -> libp2p::core::transport::Boxed<(PeerId, StreamMuxerBox)> {
    let tcp = libp2p::tcp::TokioTcpConfig::new().nodelay(true);
    let transport = libp2p::dns::TokioDnsConfig::system(tcp).unwrap_or_else(|e| {
        error!("Failed to configure DNS: {}", e);
        exit(1);
    });

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

pub(crate) async fn build_network_behaviour(
    enr: Enr,
    enr_key: CombinedKey,
    network_config: NetworkConfig,
) -> BehaviourComposer {
    let mut discovery = crate::discovery::behaviour::Behaviour::new(enr, enr_key).await;
    // start searching for peers
    discovery.discover_peers();

    let beacon_chain = BeaconChain::new(
        network_config.chain_spec().expect("chain spec"),
        network_config
            .genesis_beacon_state()
            .expect("genesis beacon state"),
    )
    .expect("beacon chain");

    let fork_context = Arc::new(ForkContext::new::<MainnetEthSpec>(
        beacon_chain.slot(),
        beacon_chain.genesis_validators_root,
        &beacon_chain.chain_spec,
    ));

    BehaviourComposer::new(
        discovery,
        crate::peer_manager::PeerManager::new(TARGET_PEERS_COUNT),
        crate::rpc::behaviour::Behaviour::new(fork_context),
        beacon_chain,
    )
}
