use discv5::enr::CombinedPublicKey;
use discv5::Enr;
use libp2p::multiaddr::Protocol;
use libp2p::{Multiaddr, PeerId};
use tiny_keccak::{Hasher, Keccak};

// SEE: https://github.com/sigp/lighthouse/blob/4af6fcfafd2c29bca82474ee378cda9ac254783a/beacon_node/eth2_libp2p/src/discovery/enr_ext.rs#L200
pub(crate) fn enr_to_peer_id(enr: &Enr) -> PeerId {
    match enr.public_key() {
        CombinedPublicKey::Secp256k1(pk) => {
            let pk_bytes = pk.to_bytes();
            let libp2p_pk = libp2p::identity::secp256k1::PublicKey::try_from_bytes(&pk_bytes)
                .expect("valid public key");
            let public_key: libp2p::identity::PublicKey = libp2p_pk.into();
            PeerId::from(public_key)
        }
        CombinedPublicKey::Ed25519(_) => unreachable!(), // not implemented as the ENR key is generated with secp256k1
    }
}

// SEE: https://github.com/sigp/lighthouse/blob/unstable/beacon_node/lighthouse_network/src/discovery/enr_ext.rs#L240-L274
pub(crate) fn peer_id_to_node_id(peer_id: &PeerId) -> Result<discv5::enr::NodeId, String> {
    // A libp2p peer id byte representation should be 2 length bytes + 4 protobuf bytes + compressed pk bytes
    // if generated from a PublicKey with Identity multihash.
    let pk_bytes = &peer_id.to_bytes()[2..];

    let public_key = libp2p::identity::PublicKey::try_decode_protobuf(pk_bytes).map_err(|e| {
        format!(
            " Cannot parse libp2p public key public key from peer id: {}",
            e
        )
    })?;

    // TODO: matching public_key.key_type() after libp2p upgrading.
    // ref: https://github.com/sigp/lighthouse/blob/8dff926c70b7c7558e7c41316770d22608dbba4c/beacon_node/lighthouse_network/src/discovery/enr_ext.rs#L266-L293
    if let Ok(pk) = public_key.clone().try_into_secp256k1() {
        let uncompressed_key_bytes = &pk.to_bytes_uncompressed()[1..];
        let mut output = [0_u8; 32];
        let mut hasher = Keccak::v256();
        hasher.update(uncompressed_key_bytes);
        hasher.finalize(&mut output);
        Ok(discv5::enr::NodeId::parse(&output).expect("Must be correct length"))
    } else if let Ok(pk) = public_key.clone().try_into_ed25519() {
        let uncompressed_key_bytes = pk.to_bytes();
        let mut output = [0_u8; 32];
        let mut hasher = Keccak::v256();
        hasher.update(&uncompressed_key_bytes);
        hasher.finalize(&mut output);
        Ok(discv5::enr::NodeId::parse(&output).expect("Must be correct length"))
    } else {
        Err(format!(
            "Unsupported public key (Ecdsa) from peer {}",
            peer_id
        ))
    }
}

// SEE: https://github.com/sigp/lighthouse/blob/4af6fcfafd2c29bca82474ee378cda9ac254783a/beacon_node/eth2_libp2p/src/discovery/enr_ext.rs#L174
pub(crate) fn enr_to_multiaddrs(enr: &Enr) -> Vec<Multiaddr> {
    let mut multiaddrs: Vec<Multiaddr> = Vec::new();
    if let Some(ip) = enr.ip4() {
        if let Some(tcp) = enr.tcp4() {
            let mut multiaddr: Multiaddr = ip.into();
            multiaddr.push(Protocol::Tcp(tcp));
            multiaddrs.push(multiaddr);
        }
    }
    if let Some(ip6) = enr.ip6() {
        if let Some(tcp6) = enr.tcp6() {
            let mut multiaddr: Multiaddr = ip6.into();
            multiaddr.push(Protocol::Tcp(tcp6));
            multiaddrs.push(multiaddr);
        }
    }
    multiaddrs
}
