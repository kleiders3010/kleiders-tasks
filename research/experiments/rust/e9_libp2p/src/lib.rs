//! Size probe: libp2p with QUIC + TCP/noise/yamux, relay client (for DCUtR coordination),
//! DCUtR hole punching, identify (observed address), Kademlia DHT.
use libp2p::{dcutr, identify, kad, noise, relay, swarm::NetworkBehaviour, tcp, yamux, Multiaddr};
use std::ffi::c_int;

#[derive(NetworkBehaviour)]
struct B {
    relay: relay::client::Behaviour,
    dcutr: dcutr::Behaviour,
    identify: identify::Behaviour,
    kad: kad::Behaviour<kad::store::MemoryStore>,
}

#[unsafe(no_mangle)]
pub extern "C" fn probe_libp2p(port: c_int) -> c_int {
    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
    rt.block_on(async move {
        let mut swarm = libp2p::SwarmBuilder::with_new_identity()
            .with_tokio()
            .with_tcp(tcp::Config::default(), noise::Config::new, yamux::Config::default).unwrap()
            .with_quic()
            .with_relay_client(noise::Config::new, yamux::Config::default).unwrap()
            .with_behaviour(|key, relay| {
                let id = key.public().to_peer_id();
                B {
                    relay,
                    dcutr: dcutr::Behaviour::new(id),
                    identify: identify::Behaviour::new(identify::Config::new("/kt/1".into(), key.public())),
                    kad: kad::Behaviour::new(id, kad::store::MemoryStore::new(id)),
                }
            }).unwrap()
            .build();
        let addr: Multiaddr = format!("/ip4/0.0.0.0/udp/{port}/quic-v1").parse().unwrap();
        swarm.listen_on(addr).unwrap();
        use libp2p::futures::StreamExt;
        let _ = tokio::time::timeout(std::time::Duration::from_millis(10), swarm.select_next_some()).await;
        swarm.connected_peers().count() as c_int
    })
}
