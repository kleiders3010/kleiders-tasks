//! Whereabouts over a local Mainline DHT testnet (loopback), then P2P using them: no
//! Nostr involved at all after joining.

use std::net::Ipv4Addr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use kt_engine::replica::Replica;
use kt_engine::sandbox::SandboxConfig;
use kt_net::dht::DhtDiscovery;
use kt_net::p2p::{P2p, P2pConfig};
use kt_net::sync::Syncer;
use mainline::Testnet;

fn replica(tag: &str) -> Replica {
    let root = std::env::temp_dir().join(format!("kt-dht-{tag}-{}-{}", std::process::id(), rand::random::<u32>()));
    let sb = SandboxConfig { worker: PathBuf::from(env!("CARGO_BIN_EXE_kt-net-worker")), timeout: Duration::from_secs(20) };
    Replica::open(&root.join("identity"), &root.join("data"), sb).unwrap()
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn members_find_each_other_on_the_dht_and_sync_directly() {
    let testnet = Testnet::builder(10).build().unwrap();
    let (mut ra, mut rb, outsider) = (replica("a"), replica("b"), replica("x"));
    let g = ra.create_group().unwrap();
    let d = ra.create_doc(&g).unwrap();
    ra.doc(&g, &d).unwrap().get_map("tasks").insert("t0", "start").unwrap();
    ra.commit(&g, &d).unwrap();
    ra.add_member(&g, rb.device_public()).unwrap();
    rb.accept_group(g);
    for m in ra.export_group(&g).unwrap() {
        let _ = rb.receive(&m);
    }
    let loopback = P2pConfig { bind: Some("127.0.0.1:0".parse().unwrap()), ..Default::default() };
    let (sa, sb) = (Syncer::new(Arc::new(Mutex::new(ra)), vec![]), Syncer::new(Arc::new(Mutex::new(rb)), vec![]));
    let pa = P2p::start(sa.clone(), loopback.clone()).await.unwrap();
    let pb = P2p::start(sb.clone(), loopback).await.unwrap();
    pa.publish_whereabouts();
    pb.publish_whereabouts();
    let bs = Some(testnet.bootstrap.clone());
    let da = DhtDiscovery::new(sa.replica(), bs.clone(), Some(Ipv4Addr::LOCALHOST)).unwrap();
    let db = DhtDiscovery::new(sb.replica(), bs.clone(), Some(Ipv4Addr::LOCALHOST)).unwrap();
    assert_eq!(da.publish().await.published, 1);
    assert_eq!(db.publish().await.published, 1);
    let r = db.lookup().await;
    assert_eq!((r.found, r.applied), (1, 1), "B found A's sealed record");
    da.lookup().await;

    // An outsider in the same DHT can't compute the location, so it looks up nothing.
    let dx = DhtDiscovery::new(Arc::new(Mutex::new(outsider)), bs, Some(Ipv4Addr::LOCALHOST)).unwrap();
    assert_eq!(dx.lookup().await.found, 0);

    // With addresses from the DHT alone, A and B sync directly.
    sa.replica().lock().unwrap().doc(&g, &d).unwrap().get_map("tasks").insert("t1", "found via DHT").unwrap();
    sa.replica().lock().unwrap().commit(&g, &d).unwrap();
    let rep = pa.sync_round().await;
    assert_eq!(rep.peers_reached, 1, "{rep:?}");
    let v = format!("{:?}", sb.replica().lock().unwrap().doc(&g, &d).unwrap().get_deep_value());
    assert!(v.contains("found via DHT"), "{v}");
    let _ = pb;
}
