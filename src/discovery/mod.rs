pub(crate) mod behaviour;

use enr::{CombinedKey, Enr};
use std::str::FromStr;

// SEE: https://github.com/sigp/lighthouse/blob/stable/common/eth2_network_config/built_in_network_configs/kiln/boot_enr.yaml
const BOOT_ENRS: [&str; 3] = [
    "enr:-Iq4QMCTfIMXnow27baRUb35Q8iiFHSIDBJh6hQM5Axohhf4b6Kr_cOCu0htQ5WvVqKvFgY28893DHAg8gnBAXsAVqmGAX53x8JggmlkgnY0gmlwhLKAlv6Jc2VjcDI1NmsxoQK6S-Cii_KmfFdUJL2TANL3ksaKUnNXvTCv1tLwXs0QgIN1ZHCCIyk",
    "enr:-KG4QFkPJUFWuONp5grM94OJvNht9wX6N36sA4wqucm6Z02ECWBQRmh6AzndaLVGYBHWre67mjK-E0uKt2CIbWrsZ_8DhGV0aDKQc6pfXHAAAHAyAAAAAAAAAIJpZIJ2NIJpcISl6LTmiXNlY3AyNTZrMaEDHlSNOgYrNWP8_l_WXqDMRvjv6gUAvHKizfqDDVc8feaDdGNwgiMog3VkcIIjKA",
    "enr:-MK4QI-wkVW1PxL4ksUM4H_hMgTTwxKMzvvDMfoiwPBuRxcsGkrGPLo4Kho3Ri1DEtJG4B6pjXddbzA9iF2gVctxv42GAX9v5WG5h2F0dG5ldHOIAAAAAAAAAACEZXRoMpBzql9ccAAAcDIAAAAAAAAAgmlkgnY0gmlwhKRcjMiJc2VjcDI1NmsxoQK1fc46pmVHKq8HNYLkSVaUv4uK2UBsGgjjGWU6AAhAY4hzeW5jbmV0cwCDdGNwgiMog3VkcIIjKA",
];

fn boot_enrs() -> Vec<Enr<CombinedKey>> {
    BOOT_ENRS
        .iter()
        .map(|&str| Enr::from_str(str).expect("Failed to parse ENR"))
        .collect()
}

// SEE: https://github.com/sigp/lighthouse/blob/4af6fcfafd2c29bca82474ee378cda9ac254783a/beacon_node/eth2_libp2p/src/discovery/enr_ext.rs#L49
// pub(crate) fn boot_multiaddrs() -> Vec<Multiaddr> {
//     let mut multiaddrs: Vec<Multiaddr> = vec![];
//     for enr in boot_enrs() {
//         if let Some(ip) = enr.ip() {
//             if let Some(udp) = enr.udp() {
//                 let mut multiaddr: Multiaddr = ip.into();
//                 multiaddr.push(Protocol::Udp(udp));
//                 multiaddrs.push(multiaddr);
//             }
//
//             if let Some(tcp) = enr.tcp() {
//                 let mut multiaddr: Multiaddr = ip.into();
//                 multiaddr.push(Protocol::Tcp(tcp));
//                 multiaddrs.push(multiaddr);
//             }
//         }
//         if let Some(ip6) = enr.ip6() {
//             if let Some(udp6) = enr.udp6() {
//                 let mut multiaddr: Multiaddr = ip6.into();
//                 multiaddr.push(Protocol::Udp(udp6));
//                 multiaddrs.push(multiaddr);
//             }
//
//             if let Some(tcp6) = enr.tcp6() {
//                 let mut multiaddr: Multiaddr = ip6.into();
//                 multiaddr.push(Protocol::Tcp(tcp6));
//                 multiaddrs.push(multiaddr);
//             }
//         }
//     }
//
//     // ignore UDP multiaddr as we using TcpConfig of libp2p.
//     // SEE: https://github.com/sigp/lighthouse/blob/0aee7ec873bcc7206b9acf2741f46c209b510c57/beacon_node/eth2_libp2p/src/service.rs#L211
//     multiaddrs
//         .into_iter()
//         .filter(|addr| {
//             let components = addr.iter().collect::<Vec<_>>();
//             if let Protocol::Udp(_) = components[1] {
//                 false
//             } else {
//                 true
//             }
//         })
//         .collect()
// }
