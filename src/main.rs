mod beacon_chain;
mod behaviour;
mod bootstrap;
mod config;
mod discovery;
mod identity;
mod network;
mod peer_db;
mod peer_manager;
mod rpc;
mod signal;
mod sync;
mod types;

use crate::beacon_chain::BeaconChain;
use crate::behaviour::{BehaviourComposer, BehaviourComposerEvent};
use crate::bootstrap::{build_network_behaviour, build_network_transport};
use crate::config::NetworkConfig;
use crate::network::Network;
use crate::peer_db::PeerDB;
use ::types::MainnetEthSpec;
use beacon_node::{ClientBuilder, ClientConfig, ClientGenesis};
use discv5::enr::{CombinedKey, EnrBuilder};
use environment::{EnvironmentBuilder, LoggerConfig};
use eth2_network_config::Eth2NetworkConfig;
use libp2p::identity::Keypair;
use parking_lot::RwLock;
use std::sync::Arc;
use tracing::info;

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

    // BeaconChain from lighthouse
    let eth2_network_config = Eth2NetworkConfig::constant("prater")
        .expect("Initiating the network config never fail")
        .expect("wrong network name");

    let mut environment = EnvironmentBuilder::mainnet()
        .initialize_logger(LoggerConfig::default())
        .expect("initialize_logger")
        .multi_threaded_tokio_runtime()
        .expect("multi_threaded_tokio_runtime")
        .optional_eth2_network_config(Some(eth2_network_config))
        .expect("optional_eth2_network_config")
        .build()
        .expect("environment builder");

    let lh_beacon_chain = runtime.block_on(async {
        let client_config = ClientConfig::default();
        let db_path = client_config.create_db_path().expect("db_path");
        let freezer_db_path = client_config
            .create_freezer_db_path()
            .expect("freezer_db_path");

        let runtime_context = environment.core_context();

        let client_builder = ClientBuilder::new(MainnetEthSpec)
            .chain_spec(runtime_context.eth2_config.spec.clone())
            .runtime_context(runtime_context.clone())
            .disk_store(
                &db_path,
                &freezer_db_path,
                client_config.store.clone(),
                runtime_context.log().clone(),
            )
            .expect("disk_store")
            .beacon_chain_builder(
                ClientGenesis::SszBytes {
                    genesis_state_bytes: network_config.genesis_state_bytes.clone(),
                },
                client_config,
            )
            .await
            .expect("beacon_chain_builder")
            .system_time_slot_clock()
            .expect("")
            .dummy_eth1_backend()
            .expect("")
            .build_beacon_chain()
            .expect("build_beacon_chain");

        client_builder.beacon_chain.expect("beacon_chain")
    });

    // BeaconChain
    let beacon_chain = Arc::new(RwLock::new(
        BeaconChain::new(
            network_config.chain_spec().expect("chain spec"),
            network_config
                .genesis_beacon_state()
                .expect("genesis beacon state"),
        )
        .expect("beacon chain"),
    ));

    // SyncManager
    let sync_sender = sync::spawn(runtime.clone(), peer_db.clone(), beacon_chain.clone());

    // Network
    let network = runtime.block_on(Network::new(
        lh_beacon_chain,
        beacon_chain,
        sync_sender,
        key_pair,
        enr,
        enr_key,
        network_config,
        peer_db,
        runtime.clone(),
    ));

    runtime.block_on(network.spawn(runtime.clone()));

    // block until shutdown requested
    let message = crate::signal::block_until_shutdown_requested(runtime);

    info!("Shutting down: {:?}", message.0);
}
