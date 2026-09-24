//! Independent verification of R3 claims against loro 1.16.2 (local only).
use loro::{ExportMode, Frontiers, LoroDoc, UndoManager, ID};

fn doc(peer: u64) -> LoroDoc {
    let d = LoroDoc::new();
    d.set_peer_id(peer).unwrap();
    d
}

fn t1_forkat_on_shallow() {
    println!("\n== T1 fork_at / SnapshotAt / checkout on shallow doc");
    let a = doc(1);
    let m = a.get_map("m");
    for i in 0..10 {
        m.insert(&format!("k{i}"), i as i64).unwrap();
        a.commit();
    }
    let mid = Frontiers::from_id(ID::new(1, 4));
    let full_fork = a.fork_at(&mid);
    println!("full doc fork_at(mid): ok={}", full_fork.is_ok());
    let root = Frontiers::from_id(ID::new(1, 6));
    let sh = a.export(ExportMode::shallow_snapshot(&root)).unwrap();
    let s = LoroDoc::from_snapshot(&sh).unwrap();
    println!("shallow is_shallow={} since={:?}", s.is_shallow(), s.shallow_since_frontiers());
    let after = Frontiers::from_id(ID::new(1, 8));
    println!("shallow fork_at(after root) -> {:?}", s.fork_at(&after).err());
    println!("shallow fork_at(head) -> {:?}", s.fork_at(&s.oplog_frontiers()).err());
    println!(
        "shallow SnapshotAt(after) -> {:?}",
        s.export(ExportMode::SnapshotAt { version: std::borrow::Cow::Owned(after.clone()) }).err()
    );
    println!("shallow checkout(before root) -> {:?}", s.checkout(&mid).err());
    println!("shallow checkout(after root) -> {:?}", s.checkout(&after).err());
    s.checkout_to_latest();
    let so = s.export(ExportMode::StateOnly(Some(std::borrow::Cow::Owned(after.clone()))));
    println!("shallow StateOnly(after root) -> ok={} {:?}", so.is_ok(), so.as_ref().err());
    let so2 = s.export(ExportMode::StateOnly(Some(std::borrow::Cow::Owned(mid.clone()))));
    println!("shallow StateOnly(before root) -> ok={} {:?}", so2.is_ok(), so2.as_ref().err());
}

fn t2_concurrent_with_root() {
    println!("\n== T2 update concurrent with shallow root; partial apply; pending forever");
    let a = doc(1);
    a.get_map("m").insert("base", 0).unwrap();
    a.commit();
    let b = doc(2);
    b.import(&a.export(ExportMode::Snapshot).unwrap()).unwrap();
    b.get_map("m").insert("b1", "offline edit").unwrap();
    b.commit();
    a.get_map("m").insert("a1", 1).unwrap();
    a.commit();
    let root = a.oplog_frontiers();
    let sh = a.export(ExportMode::shallow_snapshot(&root)).unwrap();
    let c = doc(3);
    c.import(&sh).unwrap();
    println!("C shallow since={:?} value={:?}", c.shallow_since_frontiers(), c.get_deep_value());
    // D has everything, plus A's post-root change a2
    let d = doc(4);
    d.import(&a.export(ExportMode::Snapshot).unwrap()).unwrap();
    d.import(&b.export(ExportMode::all_updates()).unwrap()).unwrap();
    a.get_map("m").insert("a2", 2).unwrap();
    a.commit();
    d.import(&a.export(ExportMode::all_updates()).unwrap()).unwrap();
    let blob = d.export(ExportMode::updates(&c.oplog_vv())).unwrap();
    let r = c.import(&blob);
    println!("C import(blob with a2 + b1) -> {:?}", r);
    println!("C value after: {:?}", c.get_deep_value());
    // B keeps editing; b2 depends on b1 which C rejected
    b.get_map("m").insert("b2", "second").unwrap();
    b.commit();
    let b2 = b.export(ExportMode::updates(&d.oplog_vv())).unwrap();
    let r2 = c.import(&b2);
    println!("C import(b2 alone) -> {:?}", r2);
    println!("C value after b2: {:?}", c.get_deep_value());
    // Brand-new peer that never synced
    let f = doc(5);
    f.get_map("m").insert("fresh", "never synced").unwrap();
    f.commit();
    println!("C import(genesis change of never-synced peer) -> {:?}", c.import(&f.export(ExportMode::all_updates()).unwrap()));
    println!("C value: {:?}", c.get_deep_value());
}

fn t3_stale_device_imports_shallow() {
    println!("\n== T3 stale device (older than root) imports shallow snapshot into its non-empty doc");
    let a = doc(1);
    let m = a.get_map("m");
    m.insert("base", 0).unwrap();
    a.commit();
    // E: stale but no local edits
    let e = doc(6);
    e.import(&a.export(ExportMode::Snapshot).unwrap()).unwrap();
    // B: stale with unsynced local edit
    let b = doc(2);
    b.import(&a.export(ExportMode::Snapshot).unwrap()).unwrap();
    b.get_map("m").insert("b1", "offline").unwrap();
    b.commit();
    for i in 0..10 {
        m.insert(&format!("a{i}"), i as i64).unwrap();
        a.commit();
    }
    // root somewhere in the middle of A's run, so there are retained ops after it
    let root = Frontiers::from_id(ID::new(1, 5));
    let sh = a.export(ExportMode::shallow_snapshot(&root)).unwrap();
    let re = e.import(&sh);
    println!("E(no local edits) import shallow -> {:?}", re);
    println!("E value: {:?}", e.get_deep_value());
    let rb = b.import(&sh);
    println!("B(local edit) import shallow -> {:?}", rb);
    println!("B value: {:?}", b.get_deep_value());
    // root at head: no retained ops
    let sh2 = a.export(ExportMode::shallow_snapshot(&a.oplog_frontiers())).unwrap();
    let e2 = doc(7);
    e2.import(&LoroDoc::from_snapshot(&a.fork_at(&Frontiers::from_id(ID::new(1, 0))).unwrap().export(ExportMode::Snapshot).unwrap()).unwrap().export(ExportMode::Snapshot).unwrap()).unwrap();
    println!("E2 before: {:?}", e2.get_deep_value());
    println!("E2 import shallow@head -> {:?}", e2.import(&sh2));
    println!("E2 value: {:?}", e2.get_deep_value());
}

fn t4_root_lands_earlier() {
    println!("\n== T4 requested frontier vs actual shallow root with concurrent heads");
    let a = doc(1);
    a.get_map("m").insert("base", 0).unwrap();
    a.commit();
    let b = doc(2);
    b.import(&a.export(ExportMode::Snapshot).unwrap()).unwrap();
    b.get_map("m").insert("b", 1).unwrap();
    b.commit();
    for i in 0..50 {
        a.get_map("m").insert(&format!("a{i}"), i as i64).unwrap();
        a.commit();
    }
    a.import(&b.export(ExportMode::all_updates()).unwrap()).unwrap();
    println!("A frontiers (2 heads, no merge commit): {:?}", a.oplog_frontiers());
    let req = Frontiers::from_id(ID::new(1, 50));
    let s1 = LoroDoc::from_snapshot(&a.export(ExportMode::shallow_snapshot(&req)).unwrap()).unwrap();
    println!("requested {:?} -> actual root {:?}", req, s1.shallow_since_frontiers());
    let s2 = LoroDoc::from_snapshot(&a.export(ExportMode::shallow_snapshot(&a.oplog_frontiers())).unwrap()).unwrap();
    println!("requested {:?} -> actual root {:?}", a.oplog_frontiers(), s2.shallow_since_frontiers());
    // now a merge commit
    a.get_map("m").insert("merge", 1).unwrap();
    a.commit();
    let s3 = LoroDoc::from_snapshot(&a.export(ExportMode::shallow_snapshot(&a.oplog_frontiers())).unwrap()).unwrap();
    println!("after merge commit, requested {:?} -> actual root {:?}", a.oplog_frontiers(), s3.shallow_since_frontiers());
}

fn t5_peer_reuse_and_squat() {
    println!("\n== T5 PeerID reuse via updates; counter squatting");
    let v = doc(10);
    v.get_map("m").insert("x", "v0").unwrap();
    v.commit();
    let r = doc(11);
    r.import(&v.export(ExportMode::all_updates()).unwrap()).unwrap();
    // malicious M imports V's history and writes as V with V's next counter
    let mdoc = doc(12);
    mdoc.import(&v.export(ExportMode::all_updates()).unwrap()).unwrap();
    mdoc.set_peer_id(10).unwrap();
    mdoc.get_map("m").insert("x", "FORGED by M as V").unwrap();
    mdoc.commit();
    println!("R import forged -> {:?}", r.import(&mdoc.export(ExportMode::updates(&r.oplog_vv())).unwrap()));
    // V's genuine next edit at the same counter
    v.get_map("m").insert("x", "genuine V edit").unwrap();
    v.get_map("m").insert("y", "genuine V second key").unwrap();
    v.commit();
    println!("R import genuine V (all updates) -> {:?}", r.import(&v.export(ExportMode::all_updates()).unwrap()));
    println!("R value: {:?}", r.get_deep_value());
    println!("V value: {:?}", v.get_deep_value());
    println!("V import R's updates -> {:?}", v.import(&r.export(ExportMode::all_updates()).unwrap()));
    println!("V value after: {:?}  converged={}", v.get_deep_value(), v.get_deep_value() == r.get_deep_value());
}

fn t6_tree_delete_semantics() {
    println!("\n== T6 tree: move deleted node back; undo delete identity");
    let d = doc(20);
    let t = d.get_tree("t");
    let n = t.create(None).unwrap();
    t.get_meta(n).unwrap().insert("title", "task").unwrap();
    d.commit();
    t.delete(n).unwrap();
    d.commit();
    println!("mov deleted node back to root -> {:?}", t.mov(n, None));
    let d2 = doc(21);
    let t2 = d2.get_tree("t");
    let mut um = UndoManager::new(&d2);
    let n2 = t2.create(None).unwrap();
    t2.get_meta(n2).unwrap().insert("title", "task").unwrap();
    d2.commit();
    t2.delete(n2).unwrap();
    d2.commit();
    um.undo().unwrap();
    d2.commit();
    let nodes = t2.nodes();
    let alive: Vec<_> = nodes.iter().filter(|x| !t2.is_node_deleted(x).unwrap_or(true)).collect();
    println!("original id {:?}; alive after undo {:?}; original deleted={:?}", n2, alive, t2.is_node_deleted(&n2));
}

fn t7_pending_persistence() {
    println!("\n== T7 pending changes across snapshot export/reload");
    let a = doc(30);
    a.get_map("m").insert("k1", 1).unwrap();
    a.commit();
    let u1 = a.export(ExportMode::all_updates()).unwrap();
    let vv1 = a.oplog_vv();
    a.get_map("m").insert("k2", 2).unwrap();
    a.commit();
    let u2 = a.export(ExportMode::updates(&vv1)).unwrap();
    let x = doc(31);
    println!("X import u2 first -> {:?}", x.import(&u2));
    let snap = x.export(ExportMode::Snapshot).unwrap();
    let y = LoroDoc::from_snapshot(&snap).unwrap();
    println!("Y (reloaded) import u1 -> {:?}", y.import(&u1));
    println!("Y value: {:?} (X after u1: {:?})", y.get_deep_value(), { x.import(&u1).unwrap(); x.get_deep_value() });
}

fn t8_write_counter_blob(dir: &str) {
    println!("\n== T8 write blobs with a Counter for the no-counter build");
    let d = doc(40);
    d.get_map("m").insert("title", "t").unwrap();
    d.get_counter("c").increment(5.0).unwrap();
    let child = d.get_map("m").insert_container("cnt", loro::LoroCounter::new()).unwrap();
    child.increment(3.0).unwrap();
    d.commit();
    std::fs::write(format!("{dir}/counter_snapshot.bin"), d.export(ExportMode::Snapshot).unwrap()).unwrap();
    std::fs::write(format!("{dir}/counter_updates.bin"), d.export(ExportMode::all_updates()).unwrap()).unwrap();
    println!("writer value: {:?}", d.get_deep_value());
}

fn t9_read_back(dir: &str) {
    let p = format!("{dir}/nc_resnapshot.bin");
    if let Ok(b) = std::fs::read(&p) {
        println!("\n== T9 counter build reads snapshot re-exported by no-counter build");
        let d = LoroDoc::new();
        println!("import -> {:?}", d.import(&b));
        println!("value: {:?}", d.get_deep_value());
        let d2 = LoroDoc::from_snapshot(&b);
        println!("from_snapshot ok={} value={:?}", d2.is_ok(), d2.map(|x| x.get_deep_value()).ok());
    }
    let p = format!("{dir}/nc_reupdates.bin");
    if let Ok(b) = std::fs::read(&p) {
        let d = LoroDoc::new();
        println!("updates re-exported by no-counter build -> {:?}; value {:?}", d.import(&b), d.get_deep_value());
    }
}

fn main() {
    let dir = std::env::args().nth(1).unwrap_or(".".into());
    let only = std::env::args().nth(2);
    if only.as_deref() == Some("readback") {
        t9_read_back(&dir);
        return;
    }
    t1_forkat_on_shallow();
    t2_concurrent_with_root();
    t3_stale_device_imports_shallow();
    t4_root_lands_earlier();
    t5_peer_reuse_and_squat();
    t6_tree_delete_semantics();
    t7_pending_persistence();
    t8_write_counter_blob(&dir);
}
