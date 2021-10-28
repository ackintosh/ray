use discv5::{Discv5ConfigBuilder, Discv5};
use enr::{CombinedKey, Enr};
use std::str::FromStr;
use libp2p::Multiaddr;
use libp2p::multiaddr::Protocol;
use tracing::{info, warn};

// SEE: https://github.com/sigp/lighthouse/blob/stable/common/eth2_network_config/built_in_network_configs/pyrmont/boot_enr.yaml
const BOOT_ENRS: [&str; 4] = [
    "enr:-Ku4QOA5OGWObY8ep_x35NlGBEj7IuQULTjkgxC_0G1AszqGEA0Wn2RNlyLFx9zGTNB1gdFBA6ZDYxCgIza1uJUUOj4Dh2F0dG5ldHOIAAAAAAAAAACEZXRoMpDVTPWXAAAgCf__________gmlkgnY0gmlwhDQPSjiJc2VjcDI1NmsxoQM6yTQB6XGWYJbI7NZFBjp4Yb9AYKQPBhVrfUclQUobb4N1ZHCCIyg",
    "enr:-Ku4QOksdA2tabOGrfOOr6NynThMoio6Ggka2oDPqUuFeWCqcRM2alNb8778O_5bK95p3EFt0cngTUXm2H7o1jkSJ_8Dh2F0dG5ldHOIAAAAAAAAAACEZXRoMpDVTPWXAAAgCf__________gmlkgnY0gmlwhDaa13aJc2VjcDI1NmsxoQKdNQJvnohpf0VO0ZYCAJxGjT0uwJoAHbAiBMujGjK0SoN1ZHCCIyg",
    "enr:-LK4QDiPGwNomqUqNDaM3iHYvtdX7M5qngson6Qb2xGIg1LwC8-Nic0aQwO0rVbJt5xp32sRE3S1YqvVrWO7OgVNv0kBh2F0dG5ldHOIAAAAAAAAAACEZXRoMpA7CIeVAAAgCf__________gmlkgnY0gmlwhBKNA4qJc2VjcDI1NmsxoQKbBS4ROQ_sldJm5tMgi36qm5I5exKJFb4C8dDVS_otAoN0Y3CCIyiDdWRwgiMo",
    "enr:-LK4QKAezYUw_R4P1vkzfw9qMQQFJvRQy3QsUblWxIZ4FSduJ2Kueik-qY5KddcVTUsZiEO-oZq0LwbaSxdYf27EjckBh2F0dG5ldHOIAAAAAAAAAACEZXRoMpA7CIeVAAAgCf__________gmlkgnY0gmlwhCOmkIaJc2VjcDI1NmsxoQOQgTD4a8-rESfTdbCG0V6Yz1pUvze02jB2Py3vzGWhG4N0Y3CCIyiDdWRwgiMo",
];

pub(crate) fn build_discv5(
    local_enr: Enr<CombinedKey>,
    local_enr_key: CombinedKey,
) -> Discv5 {
    // default configuration
    let config = Discv5ConfigBuilder::new().build();

    // construct the discv5 server
    let mut discv5 = Discv5::new(local_enr, local_enr_key, config).unwrap();

    {
        for boot_enr in boot_enrs() {
            info!("Boot ENR: {}", boot_enr);
            if let Err(e) = discv5.add_enr(boot_enr) {
                warn!("Failed to add Boot ENR: {:?}", e);
            }
        }
    }

    discv5
}

fn boot_enrs() -> Vec<Enr<CombinedKey>> {
    BOOT_ENRS.iter().map(|&str| {
        Enr::from_str(str).expect("Failed to parse ENR")
    }).collect()
}

// SEE: https://github.com/sigp/lighthouse/blob/4af6fcfafd2c29bca82474ee378cda9ac254783a/beacon_node/eth2_libp2p/src/discovery/enr_ext.rs#L49
pub(crate) fn boot_multiaddrs() -> Vec<Multiaddr> {
    let mut multiaddrs: Vec<Multiaddr> = vec![];
    for enr in boot_enrs() {
        if let Some(ip) = enr.ip() {
            if let Some(udp) = enr.udp() {
                let mut multiaddr: Multiaddr = ip.into();
                multiaddr.push(Protocol::Udp(udp));
                multiaddrs.push(multiaddr);
            }

            if let Some(tcp) = enr.tcp() {
                let mut multiaddr: Multiaddr = ip.into();
                multiaddr.push(Protocol::Tcp(tcp));
                multiaddrs.push(multiaddr);
            }
        }
        if let Some(ip6) = enr.ip6() {
            if let Some(udp6) = enr.udp6() {
                let mut multiaddr: Multiaddr = ip6.into();
                multiaddr.push(Protocol::Udp(udp6));
                multiaddrs.push(multiaddr);
            }

            if let Some(tcp6) = enr.tcp6() {
                let mut multiaddr: Multiaddr = ip6.into();
                multiaddr.push(Protocol::Tcp(tcp6));
                multiaddrs.push(multiaddr);
            }
        }
    }

    // ignore UDP multiaddr as we using TcpConfig of libp2p.
    // SEE: https://github.com/sigp/lighthouse/blob/0aee7ec873bcc7206b9acf2741f46c209b510c57/beacon_node/eth2_libp2p/src/service.rs#L211
    multiaddrs.into_iter().filter(|addr| {
        let components = addr.iter().collect::<Vec<_>>();
        if let Protocol::Udp(_) = components[1] {
            false
        } else {
            true
        }
    }).collect()
}
