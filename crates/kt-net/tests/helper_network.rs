//! Helper network: a stranger's device keeps a group's slots in RAM while the group's
//! members are never online at the same time.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use kt_engine::replica::Replica;
use kt_engine::sandbox::SandboxConfig;
use kt_net::helper::{HelperLimits, HelperMailbox};
use kt_net::mailbox::Mailbox;
use kt_net::p2p::{P2p, P2pConfig};
use kt_net::sync::Syncer;

fn replica(tag: &str) -> Replica {
    let root = std::env::temp_dir().join(format!("kt-help-{tag}-{}-{}", std::process::id(), rand::random::<u32>()));
    let sb = SandboxConfig { worker: PathBuf::from(env!("CARGO_BIN_EXE_kt-net-worker")), timeout: Duration::from_secs(20) };
    Replica::open(&root.join("identity"), &root.join("data"), sb).unwrap()
}

fn cfg() -> P2pConfig {
    P2pConfig { bind: Some("127.0.0.1:0".parse().unwrap()), ..Default::default() }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_strangers_helper_carries_changes_between_members_who_never_meet() {
    // The helper: someone else's device, helping by default.
    let hs = Syncer::new(Arc::new(Mutex::new(replica("h"))), vec![]);
    let hp = P2p::start(hs.clone(), P2pConfig { helper: Some(HelperLimits { base_pow_bits: 8, ..Default::default() }), ..cfg() }).await.unwrap();
    let helper_addr = hp.where_data().to_addr().unwrap();

    let (mut ra, mut rb) = (replica("a"), replica("b"));
    let g = ra.create_group().unwrap();
    let d = ra.create_doc(&g).unwrap();
    ra.doc(&g, &d).unwrap().get_map("tasks").insert("t0", "start").unwrap();
    ra.commit(&g, &d).unwrap();
    ra.add_member(&g, rb.device_public()).unwrap();
    rb.accept_group(g);
    for m in ra.export_group(&g).unwrap() {
        let _ = rb.receive(&m);
    }
    let _ = ra.outbox();

    // A: online, edits, writes to the helper, goes offline.
    let (a_rep, b_rep) = (Arc::new(Mutex::new(ra)), Arc::new(Mutex::new(rb)));
    let a0 = Syncer::new(a_rep.clone(), vec![]);
    let ap = P2p::start(a0.clone(), cfg()).await.unwrap();
    let amb = Arc::new(HelperMailbox::new(ap.clone(), vec![helper_addr.clone()]));
    let a = Syncer::new(a_rep.clone(), vec![amb.clone() as Arc<dyn Mailbox>]);
    a_rep.lock().unwrap().doc(&g, &d).unwrap().get_map("tasks").insert("t1", "left with a helper").unwrap();
    a_rep.lock().unwrap().commit(&g, &d).unwrap();
    let w = a.write_round().await;
    assert!(w.written >= 1, "stored on the helper (after solving its proof of work): {w:?}");
    ap.close().await;
    let used = hp.helper().unwrap().info().used;
    assert!(used > 0);

    // B: comes online later, reads from the helper.
    let b0 = Syncer::new(b_rep.clone(), vec![]);
    let bp = P2p::start(b0.clone(), cfg()).await.unwrap();
    let bmb = Arc::new(HelperMailbox::new(bp.clone(), vec![helper_addr]));
    let b = Syncer::new(b_rep.clone(), vec![bmb.clone() as Arc<dyn Mailbox>]);
    let r = b.read_round().await;
    assert!(r.slots_opened >= 1, "{r:?}");
    let v = format!("{:?}", b_rep.lock().unwrap().doc(&g, &d).unwrap().get_deep_value());
    assert!(v.contains("left with a helper"), "{v}");

    // The helper holds only ciphertext for a group it isn't in.
    assert!(hs.replica().lock().unwrap().groups().is_empty());
    let rec = bmb.records();
    assert!(rec.values().all(|r| r.reachable > 0 && r.withheld == 0), "{rec:?}");
}
