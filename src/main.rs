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

use crate::behaviour::{BehaviourComposer, BehaviourComposerEvent};
use crate::bootstrap::{build_network_behaviour, build_network_transport};
use crate::config::NetworkConfig;
use crate::network::Network;
use crate::peer_db::PeerDB;
use ::types::MainnetEthSpec;
use client::config::{ClientGenesis, Config};
use client::ClientBuilder;
use discv5::enr::CombinedKey;
use discv5::Enr;
use environment::{EnvironmentBuilder, LoggerConfig};
use eth2_network_config::Eth2NetworkConfig;
use parking_lot::RwLock;
use ssz::Encode;
use std::sync::Arc;
use tracing::info;

// Target number of peers to connect to.
const TARGET_PEERS_COUNT: usize = 50;

fn main() {
    tracing_subscriber::fmt::init();
    info!("Starting Ray v{}", env!("CARGO_PKG_VERSION"));

    // Keys
    info!("Generating keys...");
    let enr_key = CombinedKey::generate_secp256k1();
    let key_pair: libp2p::identity::Keypair = {
        match enr_key {
            CombinedKey::Secp256k1(ref key) => {
                let mut key_bytes = key.to_bytes();
                let secret_key =
                    libp2p::identity::secp256k1::SecretKey::try_from_bytes(&mut key_bytes)
                        .expect("valid secp256k1 key");

                let kp: libp2p::identity::secp256k1::Keypair = secret_key.into();
                kp.into()
            }
            CombinedKey::Ed25519(_) => unreachable!(), // not implemented as the ENR key is generated with secp256k1
        }
    };
    info!("Generated ENR keys.");

    // NetworkConfig
    // Ref: https://github.com/sigp/lighthouse/blob/b6493d5e2400234ce7148e3a400d6663c3f0af89/common/clap_utils/src/lib.rs#L20
    info!("Loading NetworkConfig...");
    let network_config = NetworkConfig::new().expect("should load network config");
    info!("Loaded NetworkConfig.");

    // tokio Runtime
    info!("Building tokio runtime...");
    let runtime = Arc::new(
        tokio::runtime::Builder::new_multi_thread()
            .thread_name("ray")
            .enable_all()
            .build()
            .unwrap(),
    );
    info!("Built tokio runtime.");

    // PeerDB
    let peer_db = Arc::new(RwLock::new(PeerDB::new()));

    // Eth2NetworkConfig
    info!("Initializing Eth2NetworkConfig...");
    let eth2_network_config = Eth2NetworkConfig::constant("holesky")
        .expect("Initiating the network config never fail")
        .expect("wrong network name");
    info!(network = "holesky", "Initialized Eth2NetworkConfig.");

    // Environment
    info!("Building Environment...");
    let environment = {
        let (builder, _file_logging_layer, _stdout_logging_layer, _sse_logging_layer_opt) =
            EnvironmentBuilder::mainnet().init_tracing(LoggerConfig::default(), "beacon_node");

        builder
            .multi_threaded_tokio_runtime()
            .expect("multi_threaded_tokio_runtime")
            .eth2_network_config(eth2_network_config)
            .expect("optional_eth2_network_config")
            .build()
            .expect("environment builder")
    };
    info!(spec = "mainnet", "Built Environment.");

    // BeaconChain
    info!("Building BeaconChain...");
    let lh_beacon_chain = runtime.block_on(async {
        let client_config = {
            let mut data_dir = home::home_dir().expect("home dir");
            data_dir.push(".ray");
            info!(data_dir = ?data_dir.display(), "Building the core configuration of a beacon node.");
            let mut client_config = Config::default();
            client_config.set_data_dir(data_dir);
            client_config
        };

        let db_path = client_config.create_db_path().expect("db_path");
        let freezer_db_path = client_config
            .create_freezer_db_path()
            .expect("freezer_db_path");
        let blobs_db_path = client_config.create_blobs_db_path().expect("blob_db_path");

        let runtime_context = environment.core_context();

        let client_builder = ClientBuilder::new(MainnetEthSpec)
            .chain_spec(runtime_context.eth2_config.spec.clone())
            .runtime_context(runtime_context.clone())
            .disk_store(
                &db_path,
                &freezer_db_path,
                &blobs_db_path,
                client_config.store.clone(),
            )
            .expect("disk_store")
            .beacon_chain_builder(
                // Ethereum Beacon Chain checkpoint sync endpoints
                // https://eth-clients.github.io/checkpoint-sync-endpoints/
                ClientGenesis::CheckpointSyncUrl {
                    url: "https://checkpoint-sync.holesky.ethpandaops.io/"
                        .parse()
                        .expect("checkpoint sync url should be parsed correctly."),
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
    info!("Built BeaconChain.");

    let (network_sender, network_receiver) = tokio::sync::mpsc::unbounded_channel();

    // SyncManager
    info!("Building SyncManager...");
    let sync_sender = sync::spawn(
        runtime.clone(),
        peer_db.clone(),
        lh_beacon_chain.clone(),
        network_sender,
    );
    info!("Built and spawned SyncManager.");

    // construct a local ENR
    // TODO: update local ENR on a new fork
    // https://github.com/sigp/lighthouse/blob/878027654f0ebc498168c7d9f0646fc1d7f5d710/beacon_node/network/src/service.rs#L483
    let enr_fork_id = lh_beacon_chain.enr_fork_id();
    let enr = Enr::builder()
        .add_value("eth2", &enr_fork_id.as_ssz_bytes())
        .build(&enr_key)
        .unwrap();
    info!("Local ENR: {}", enr);

    // Network
    info!("Building Network...");
    let network = runtime.block_on(Network::new(
        network_receiver,
        lh_beacon_chain,
        sync_sender,
        key_pair,
        enr,
        enr_key,
        network_config,
        peer_db,
        runtime.clone(),
    ));
    runtime.block_on(network.spawn(runtime.clone()));
    info!("Built and spawned Network");

    // block until shutdown requested
    let message = crate::signal::block_until_shutdown_requested(runtime);

    info!("Shutting down: {:?}", message.0);
}
