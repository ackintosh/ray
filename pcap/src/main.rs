use pcap::Direction;

fn main() {
    // get the default Device
    let device = pcap::Device::lookup()
        .expect("device lookup failed")
        .expect("no device available");
    println!("Using device {}", device.name);

    // Setup Capture
    let mut cap = pcap::Capture::from_device(device)
        .unwrap()
        .immediate_mode(true)
        .open()
        .unwrap();

    cap.filter("tcp",  true).unwrap();
    cap.direction(Direction::In).unwrap();
    let link_type = cap.get_datalink();
    println!("link type: {link_type:?}");

    let mut count = 0;
    cap.for_each(None, |packet| {
        println!("Got {:?}", packet.header);
        count += 1;
        if count > 10 {
            panic!("ow");
        }
    })
    .unwrap();
}
