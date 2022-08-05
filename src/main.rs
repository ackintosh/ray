mod beacon_chain;
mod behaviour;
mod bootstrap;
mod config;
mod discovery;
mod identity;
mod peer_db;
mod peer_manager;
mod rpc;
mod signal;
mod sync;
mod types;

use crate::beacon_chain::BeaconChain;
use crate::behaviour::BehaviourComposer;
use crate::bootstrap::{build_network_behaviour, build_network_transport};
use crate::config::NetworkConfig;
use crate::peer_db::PeerDB;
use discv5::enr::{CombinedKey, EnrBuilder};
use futures::StreamExt;
use libp2p::identity::Keypair;
use libp2p::swarm::SwarmBuilder;
use parking_lot::RwLock;
use std::future::Future;
use std::pin::Pin;
use std::process::exit;
use std::sync::{Arc, Weak};
use tokio::runtime::Runtime;
use tracing::{error, info, warn};

// Target number of peers to connect to.
const TARGET_PEERS_COUNT: usize = 50;

fn main() {
    tracing_subscriber::fmt::init();
    info!("Ray v{}", env!("CARGO_PKG_VERSION"));

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

    // Load network configs
    // Ref: https://github.com/sigp/lighthouse/blob/b6493d5e2400234ce7148e3a400d6663c3f0af89/common/clap_utils/src/lib.rs#L20
    let network_config = NetworkConfig::new().expect("should load network configs");

    // build the tokio executor
    let runtime = Arc::new(
        tokio::runtime::Builder::new_multi_thread()
            .thread_name("ray")
            .enable_all()
            .build()
            .unwrap(),
    );

    // PeerDB
    let peer_db = Arc::new(RwLock::new(PeerDB::new()));

    // Sync
    let sync_sender = sync::spawn(runtime.clone(), peer_db.clone());

    // libp2p
    // Ref: https://github.com/sigp/lighthouse/blob/0aee7ec873bcc7206b9acf2741f46c209b510c57/beacon_node/eth2_libp2p/src/service.rs#L66
    let local_peer_id = crate::identity::enr_to_peer_id(&enr);
    info!("Local PeerId: {}", local_peer_id);
    let transport = runtime.block_on(build_network_transport(key_pair));
    let behaviour = runtime.block_on(build_network_behaviour(
        enr,
        enr_key,
        network_config,
        sync_sender,
        peer_db,
    ));

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
    let mut swarm = SwarmBuilder::new(transport, behaviour, local_peer_id)
        .executor(Box::new(Executor(Arc::downgrade(&runtime))))
        .build();

    runtime.spawn(async move {
        let listen_multiaddr = {
            let mut multiaddr =
                libp2p::core::multiaddr::Multiaddr::from(std::net::Ipv4Addr::new(0, 0, 0, 0));
            multiaddr.push(libp2p::core::multiaddr::Protocol::Tcp(9000));
            multiaddr
        };

        match swarm.listen_on(listen_multiaddr.clone()) {
            Ok(_) => {
                info!("Listening established: {}", listen_multiaddr);
            }
            Err(e) => {
                error!("{}", e);
                exit(1);
            }
        }

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
}
