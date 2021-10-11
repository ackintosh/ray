use std::net::SocketAddr;
use std::process::exit;
use discv5::enr::EnrBuilder;
use discv5::{Discv5ConfigBuilder, Discv5, Discv5Event};
use enr::{CombinedKey, Enr, NodeId};
use std::str::FromStr;
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

    runtime.block_on(
        async {
            loop {
                tokio::select! {
                    Some(event) = event_stream.recv() => {
                        match event {
                            Discv5Event::Discovered(enr) => {
                                info!("{}", enr);
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    )

    // TODO: discv5.shutdown();
}
