mod beacon_chain;
mod behaviour;
mod discovery;
mod identity;
mod peer_manager;
mod rpc;
mod signal;
mod types;

use crate::behaviour::BehaviourComposer;
use discv5::enr::EnrBuilder;
use enr::CombinedKey;
use futures::StreamExt;
use libp2p::identity::Keypair;
use libp2p::noise;
use libp2p::swarm::SwarmBuilder;
use libp2p::Transport;
use std::future::Future;
use std::pin::Pin;
use std::process::exit;
use std::sync::{Arc, Weak};
use tokio::runtime::Runtime;
use tracing::{error, info, warn};
use crate::beacon_chain::BeaconChain;

fn main() {
    tracing_subscriber::fmt::init();
    info!("Ray v0.0.1");

    // generate private key
    let enr_key = CombinedKey::generate_secp256k1();
    let key_pair = {
        match enr_key {
            CombinedKey::Secp256k1(ref key) => {
                let mut key_bytes = key.to_bytes();
                let secret_key =
                    libp2p::core::identity::secp256k1::SecretKey::from_bytes(&mut key_bytes)
                        .expect("valid secp256k1 key");
                let kp: libp2p::core::identity::secp256k1::Keypair = secret_key.into();
                Keypair::Secp256k1(kp)
            }
            CombinedKey::Ed25519(_) => unreachable!(), // not implemented as the ENR key is generated with secp256k1
        }
    };

    // construct a local ENR
    let enr = EnrBuilder::new("v4").build(&enr_key).unwrap();
    info!("Local ENR: {}", enr);

    // local PeerId
    let local_peer_id = crate::identity::enr_to_peer_id(&enr);
    info!("Local PeerId: {}", local_peer_id);

    // build the tokio executor
    let runtime = Arc::new(
        tokio::runtime::Builder::new_multi_thread()
            .thread_name("ray")
            .enable_all()
            .build()
            .unwrap(),
    );

    // libp2p
    // SEE: https://github.com/sigp/lighthouse/blob/0aee7ec873bcc7206b9acf2741f46c209b510c57/beacon_node/eth2_libp2p/src/service.rs#L66
    let mut swarm = {
        let transport = {
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
        };

        let mut discovery =
            runtime.block_on(crate::discovery::behaviour::Behaviour::new(enr, enr_key));
        // start searching for peers
        discovery.discover_peers();

        let behaviour = BehaviourComposer::new(
            discovery,
            crate::peer_manager::PeerManager::new(),
            crate::rpc::behaviour::Behaviour::new(),
            BeaconChain::new(),
        );

        // use the executor for libp2p
        struct Executor(Weak<Runtime>);
        impl libp2p::core::Executor for Executor {
            fn exec(&self, f: Pin<Box<dyn Future<Output = ()> + Send>>) {
                if let Some(runtime) = self.0.upgrade() {
                    info!("Executor: Spawning a task");
                    runtime.spawn(f);
                } else {
                    warn!("Executor: Couldn't spawn task. Runtime shutting down");
                }
            }
        }

        SwarmBuilder::new(transport, behaviour, local_peer_id)
            .executor(Box::new(Executor(Arc::downgrade(&runtime))))
            .build()
    };
    let listen_multiaddr = {
        let mut multiaddr =
            libp2p::core::multiaddr::Multiaddr::from(std::net::Ipv4Addr::new(0, 0, 0, 0));
        multiaddr.push(libp2p::core::multiaddr::Protocol::Tcp(9000));
        multiaddr
    };

    runtime.spawn(async move {
        match swarm.listen_on(listen_multiaddr.clone()) {
            Ok(_) => {
                info!("Listening established: {}", listen_multiaddr);
            }
            Err(e) => {
                error!("{}", e);
                exit(1);
            }
        }

        // for addr in boot_multiaddrs() {
        //     info!("Dialing boot nodes: {}", addr);
        //     match swarm.dial_addr(addr) {
        //         Ok(()) => {}
        //         Err(e) => {
        //             warn!("Failed to dial to the peer: {}", e);
        //         }
        //     }
        // }

        // SEE:
        // https://github.com/sigp/lighthouse/blob/9667dc2f0379272fe0f36a2ec015c5a560bca652/beacon_node/network/src/service.rs#L309
        // https://github.com/sigp/lighthouse/blob/0aee7ec873bcc7206b9acf2741f46c209b510c57/beacon_node/eth2_libp2p/src/service.rs#L305
        loop {
            match swarm.select_next_some().await {
                libp2p::swarm::SwarmEvent::Behaviour(_behaviour_out_event) => {
                    info!("SwarmEvent::Behaviour");
                }
                libp2p::swarm::SwarmEvent::ConnectionEstablished {
                    peer_id,
                    endpoint: _,
                    num_established: _,
                    concurrent_dial_errors: _,
                } => {
                    info!("SwarmEvent::ConnectionEstablished. peer_id: {}", peer_id);
                }
                event => {
                    info!("other event: {:?}", event);
                }
            }
        }
    });

    // block until shutdown requested
    let message = crate::signal::block_until_shutdown_requested(runtime);

    info!("Shutting down: {:?}", message.0);

    // TODO: discv5.shutdown();
}
