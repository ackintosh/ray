use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::process::exit;
use discv5::enr::EnrBuilder;
use discv5::{Discv5ConfigBuilder, Discv5, Discv5Event};
use enr::{CombinedKey, Enr, NodeId};
use std::str::FromStr;
use std::sync::{Arc, RwLock};
use std::task::{Context, Poll};
use tokio::signal::unix::{signal, Signal, SignalKind};
use tracing::{error, info, warn};

fn main() {
    tracing_subscriber::fmt::init();
    info!("Ray v0.0.1");

    let listen_addr = "0.0.0.0:19000".parse::<SocketAddr>().unwrap();

    // construct a local ENR
    let enr_key = CombinedKey::generate_secp256k1();
    let enr = EnrBuilder::new("v4").build(&enr_key).unwrap();
    info!("Local ENR: {}", enr);

    // build the tokio executor
    let mut runtime = tokio::runtime::Builder::new_multi_thread()
        .thread_name("discv5")
        .enable_all()
        .build()
        .unwrap();

    // default configuration
    let config = Discv5ConfigBuilder::new().build();

    // construct the discv5 server
    let mut discv5 = Discv5::new(enr, enr_key, config).unwrap();

    {
        // SEE: https://github.com/sigp/lighthouse/blob/stable/common/eth2_network_config/built_in_network_configs/pyrmont/boot_enr.yaml
        let enrs = [
            "enr:-Ku4QOA5OGWObY8ep_x35NlGBEj7IuQULTjkgxC_0G1AszqGEA0Wn2RNlyLFx9zGTNB1gdFBA6ZDYxCgIza1uJUUOj4Dh2F0dG5ldHOIAAAAAAAAAACEZXRoMpDVTPWXAAAgCf__________gmlkgnY0gmlwhDQPSjiJc2VjcDI1NmsxoQM6yTQB6XGWYJbI7NZFBjp4Yb9AYKQPBhVrfUclQUobb4N1ZHCCIyg",
            "enr:-Ku4QOksdA2tabOGrfOOr6NynThMoio6Ggka2oDPqUuFeWCqcRM2alNb8778O_5bK95p3EFt0cngTUXm2H7o1jkSJ_8Dh2F0dG5ldHOIAAAAAAAAAACEZXRoMpDVTPWXAAAgCf__________gmlkgnY0gmlwhDaa13aJc2VjcDI1NmsxoQKdNQJvnohpf0VO0ZYCAJxGjT0uwJoAHbAiBMujGjK0SoN1ZHCCIyg",
            "enr:-LK4QDiPGwNomqUqNDaM3iHYvtdX7M5qngson6Qb2xGIg1LwC8-Nic0aQwO0rVbJt5xp32sRE3S1YqvVrWO7OgVNv0kBh2F0dG5ldHOIAAAAAAAAAACEZXRoMpA7CIeVAAAgCf__________gmlkgnY0gmlwhBKNA4qJc2VjcDI1NmsxoQKbBS4ROQ_sldJm5tMgi36qm5I5exKJFb4C8dDVS_otAoN0Y3CCIyiDdWRwgiMo",
            "enr:-LK4QKAezYUw_R4P1vkzfw9qMQQFJvRQy3QsUblWxIZ4FSduJ2Kueik-qY5KddcVTUsZiEO-oZq0LwbaSxdYf27EjckBh2F0dG5ldHOIAAAAAAAAAACEZXRoMpA7CIeVAAAgCf__________gmlkgnY0gmlwhCOmkIaJc2VjcDI1NmsxoQOQgTD4a8-rESfTdbCG0V6Yz1pUvze02jB2Py3vzGWhG4N0Y3CCIyiDdWRwgiMo",
        ];
        for e in enrs {
            let boot_enr: Enr<CombinedKey> = Enr::from_str(e).expect("Failed to parse ENR");
            info!("Boot ENR: {}", boot_enr);
            if let Err(e) = discv5.add_enr(boot_enr) {
                warn!("Failed to add Boot ENR: {:?}", e);
            }
        }
    }

    // start the discv5 server
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

    // block until shutdown requested
    let message = runtime.block_on(async {
        let mut handles = vec![];

        match signal(SignalKind::terminate()) {
            Ok(terminate_stream) => {
                let terminate = SignalFuture::new(terminate_stream, "Received SIGTERM");
                handles.push(terminate);
            }
            Err(e) => error!("Could not register SIGTERM handler: {}", e)
        }

        match signal(SignalKind::interrupt()) {
            Ok(interrupt_stream) => {
                let interrupt = SignalFuture::new(interrupt_stream, "Received SIGINT");
                handles.push(interrupt);
            }
            Err(e) => error!("Could not register SIGINT handler: {}", e)
        }

        futures::future::select_all(handles.into_iter()).await
    });

    info!("Shutting down: {:?}", message.0);
    info!("peers: {:?}", peers.read().unwrap());


    // TODO: discv5.shutdown();
}

// SEE: https://github.com/sigp/lighthouse/blob/d9910f96c5f71881b88eec15253b31890bcd28d2/lighthouse/environment/src/lib.rs#L492
#[cfg(target_family = "unix")]
struct SignalFuture {
    signal: Signal,
    message: &'static str,
}

#[cfg(target_family = "unix")]
impl SignalFuture {
    pub fn new(signal: Signal, message: &'static str) -> SignalFuture {
        SignalFuture { signal, message }
    }
}

#[cfg(target_family = "unix")]
impl Future for SignalFuture {
    type Output = Option<&'static str>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.signal.poll_recv(cx) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Some(_)) => Poll::Ready(Some(self.message)),
            Poll::Ready(None) => Poll::Ready(None),
        }
    }
}
