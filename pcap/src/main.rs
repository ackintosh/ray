use std::net::{IpAddr, Ipv4Addr};
use etherparse::{NetSlice, SlicedPacket, TransportSlice};
use pcap::{Direction, Linktype, Packet};

#[derive(Debug)]
struct Addr {
    ip: Ipv4Addr,
    port: u16,
}

#[derive(Debug)]
struct TcpDataInfo {
    src: Addr,
    dest: Addr,
    data_offset: usize,
    // flags: TcpMeta,
    // ts: SystemTime,
}

// TODO: load private key
// TODO: Noise decryption
// https://github.com/sigp/lighthouse/blob/bcff4aa825c4d70a215e1f229a0d1798d697fb5b/beacon_node/lighthouse_network/src/service/utils.rs#L58
// TODO: Use lighthouse codec
fn main() {
    // get the default Device
    let device = pcap::Device::lookup()
        .expect("device lookup failed")
        .expect("no device available");
    let local_addresses = device.addresses.iter().map(|addr| addr.addr).collect::<Vec<_>>();
    println!("Using device {:?}", device);
    println!("Local addresses {:?}", local_addresses);

    // Setup Capture
    let mut cap = pcap::Capture::from_device(device)
        .unwrap()
        .immediate_mode(true)
        .open()
        .unwrap();

    cap.filter("tcp",  true).unwrap();
    // cap.direction(Direction::In).unwrap();

    let link_type = cap.get_datalink();
    if !matches!(link_type, Linktype::ETHERNET) {
        panic!("Unsupported link type: {link_type:?}");
    }

    let mut count = 0;
    loop {
        let packet = cap.next_packet().unwrap();
        // println!("Got {:?}", packet.header);

        let Some(tcp_data_info) = parse_tcp(&packet) else {
            continue;
        };

        let data = &packet.data[tcp_data_info.data_offset..];
        if data.len() > 0 {
            print_tcp(tcp_data_info, data, &local_addresses);
        } else {
            println!("empty data.");
            continue;
        }

        if count > 10 {
            break;
        }
        count += 1;
    }
}

fn parse_tcp(packet: &Packet) -> Option<TcpDataInfo> {
    if packet.header.caplen < 32 {
        return None;
    }

    let ipv4data = match skip_ethernet_header(packet.data) {
        Ok(data) => data,
        Err(e) => {
            println!("error: {e}");
            return None;
        }
    };

    let sliced = SlicedPacket::from_ip(ipv4data).unwrap();
    let Some(NetSlice::Ipv4(ipv4_slice)) = sliced.net else {
        return None;
    };
    let src_addr = ipv4_slice.header().source_addr();
    let dest_addr = ipv4_slice.header().destination_addr();

    let TransportSlice::Tcp(tcp_slice) = sliced.transport.unwrap() else {
        return None;
    };
    let src_port = tcp_slice.source_port();
    let dest_port = tcp_slice.destination_port();

    let link_bytes_len = 4;
    let data_offset = link_bytes_len + ((ipv4_slice.header().ihl() * 4) + (tcp_slice.data_offset() * 4)) as usize;

    Some(TcpDataInfo {
        src: Addr {
            ip: src_addr,
            port: src_port,
        },
        dest: Addr {
            ip: dest_addr,
            port: dest_port,
        },
        data_offset,
    })
}

fn skip_ethernet_header(data: &[u8]) -> Result<&[u8], String> {
    if data.len() < 14 {
        return Err("Packet too short".to_string());
    }
    Ok(&data[14..])
}


fn print_tcp(tcp_data_info: TcpDataInfo, data: &[u8], local_addresses: &Vec<IpAddr>) {
    let is_sent = local_addresses.contains(&IpAddr::V4(tcp_data_info.src.ip));
    println!(
        "{} {}:{} -> {}:{}",
        if is_sent { "sent" } else { "recv"},
        tcp_data_info.src.ip,
        tcp_data_info.src.port,
        tcp_data_info.dest.ip,
        tcp_data_info.dest.port,
    );
}
