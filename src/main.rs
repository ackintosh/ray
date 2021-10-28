mod discovery;
mod rpc;
mod signal;

use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::process::exit;
use discv5::Discv5Event;
use discv5::enr::{CombinedPublicKey, EnrBuilder};
use enr::{CombinedKey, Enr, NodeId};
use std::sync::{Arc, RwLock, Weak};
use libp2p::swarm::{DialError, SwarmBuilder};
use libp2p::{Multiaddr, noise, PeerId};
use libp2p::identity::Keypair;
use libp2p::Transport;
use tokio::runtime::Runtime;
use tracing::{error, info, warn};
use crate::discovery::boot_multiaddrs;
use crate::rpc::behavior::Behaviour;

fn main() {
    tracing_subscriber::fmt::init();
    info!("Ray v0.0.1");

    // generate private key
    let enr_key = CombinedKey::generate_secp256k1();
    let key_pair = {
        match enr_key {
            CombinedKey::Secp256k1(ref key) => {
                let mut key_bytes = key.to_bytes();
                let secret_key = libp2p::core::identity::secp256k1::SecretKey::from_bytes(&mut key_bytes).expect("valid secp256k1 key");
                let kp: libp2p::core::identity::secp256k1::Keypair = secret_key.into();
                Keypair::Secp256k1(kp)
            }
            CombinedKey::Ed25519(_) => todo!() // not implemented as the ENR key is generated with secp256k1
        }
    };

    // construct a local ENR
    let enr = EnrBuilder::new("v4").build(&enr_key).unwrap();
    info!("Local ENR: {}", enr);

    // local PeerId
    // SEE: https://github.com/sigp/lighthouse/blob/4af6fcfafd2c29bca82474ee378cda9ac254783a/beacon_node/eth2_libp2p/src/discovery/enr_ext.rs#L200
    let local_peer_id = match enr.public_key() {
        CombinedPublicKey::Secp256k1(pk) => {
            let pk_bytes = pk.to_bytes();
            let libp2p_pk = libp2p::core::PublicKey::Secp256k1(
                libp2p::core::identity::secp256k1::PublicKey::decode(&pk_bytes).expect("valid public key")
            );
            PeerId::from(libp2p_pk)
        }
        CombinedPublicKey::Ed25519(_) => todo!() // not implemented as the ENR key is generated with secp256k1
    };
    info!("Local PeerId: {}", local_peer_id);

    // build the tokio executor
    let runtime = Arc::new(
        tokio::runtime::Builder::new_multi_thread()
            .thread_name("discv5")
            .enable_all()
            .build()
            .unwrap()
    );

    // discv5
    let mut discv5 = crate::discovery::build_discv5(enr, enr_key);
    // start the discv5 server
    let listen_addr = "0.0.0.0:19000".parse::<SocketAddr>().unwrap();
    if let Err(e) = runtime.block_on(discv5.start(listen_addr)) {
        error!("Failed to start discv5 server: {:?}", e);
        exit(1);
    }

    // establish a session by running a query
    info!("Executing bootstrap query.");
    let found = runtime.block_on(discv5.find_node(NodeId::random()));
    info!("{:?}", found);

    let mut event_stream = match runtime.block_on(discv5.event_stream()) {
        Ok(event_stream) => event_stream,
        Err(e) => {
            error!("Failed to obtain event stream: {}", e);
            exit(1);
        }
    };

    let peers: Arc<RwLock<Vec<Enr<CombinedKey>>>> = Arc::new(RwLock::new(vec![]));

    let peers_for_discv5 = peers.clone();
    runtime.spawn(async move {
        loop {
            tokio::select! {
                Some(event) = event_stream.recv() => {
                    match event {
                        Discv5Event::Discovered(enr) => {
                            info!("Discv5Event::Discovered: {}", enr);
                            peers_for_discv5.write().unwrap().push(enr);
                        }
                        Discv5Event::EnrAdded { enr, replaced } => {
                            info!("Discv5Event::EnrAdded: {}, {:?}", enr, replaced);
                        }
                        Discv5Event::TalkRequest(_)  => {}     // Ignore
                        Discv5Event::NodeInserted { node_id, replaced } => {
                            info!("Discv5Event::NodeInserted: {}, {:?}", node_id, replaced);
                        }
                        Discv5Event::SocketUpdated(socket_addr) => {
                            info!("External socket address updated: {}", socket_addr);
                        }
                    }
                }
            }
        }
    });

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

        let behaviour = Behaviour;

        // use the executor for libp2p
        struct Executor(Weak<Runtime>);
        impl libp2p::core::Executor for Executor {
            fn exec(&self, f: Pin<Box<dyn Future<Output = ()> + Send>>) {
                if let Some(runtime) = self.0.upgrade() {
                    runtime.spawn(f);
                } else {
                    warn!("Couldn't spawn task. Runtime shutting down");
                }
            }
        }

        SwarmBuilder::new(
            transport,
            behaviour,
            local_peer_id,
        )
            .executor(Box::new(Executor(Arc::downgrade(&runtime))))
            .build()
    };
    let listen_multiaddr = {
        let mut multiaddr = libp2p::core::multiaddr::Multiaddr::from(std::net::Ipv4Addr::new(0, 0, 0, 0));
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

        for addr in boot_multiaddrs() {
            info!("Dialing libp2p peer: {}", addr);
            match swarm.dial_addr(addr) {
                Ok(()) => {}
                Err(e) => {
                    warn!("Failed to dial to the peer: {}", e);
                }
            }
        }
    });

    // block until shutdown requested
    let message = crate::signal::block_until_shutdown_requested(runtime.clone());

    info!("Shutting down: {:?}", message.0);
    info!("peers: {:?}", peers.read().unwrap());

    // TODO: discv5.shutdown();
}
