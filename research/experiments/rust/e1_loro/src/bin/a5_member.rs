//! A5: malicious member attacks against Loro documents.
use loro::{ExportMode, LoroDoc};
use std::time::Instant;

fn sync(a: &LoroDoc, b: &LoroDoc) {
    let _ = b.import(&a.export(ExportMode::updates(&b.oplog_vv())).unwrap());
    let _ = a.import(&b.export(ExportMode::updates(&a.oplog_vv())).unwrap());
}

fn main() {
    println!("## 1. Impersonation: M uses victim A's PeerID");
    let a = LoroDoc::new(); a.set_peer_id(1).unwrap();
    let b = LoroDoc::new(); b.set_peer_id(2).unwrap();
    a.get_map("tasks").insert("t1", "buy milk").unwrap(); a.commit();
    sync(&a, &b);
    // M joins, syncs, then forges ops as peer 1 with counters that collide with A's next ops.
    let m = LoroDoc::from_snapshot(&b.export(ExportMode::Snapshot).unwrap()).unwrap();
    m.set_peer_id(1).unwrap();
    m.get_map("tasks").insert("t1", "FORGED by M as A").unwrap(); m.commit();
    a.get_map("tasks").insert("t2", "A's real edit").unwrap(); a.commit();
    // B receives M's forged op first, then A's real one.
    let r1 = b.import(&m.export(ExportMode::updates(&b.oplog_vv())).unwrap());
    let r2 = b.import(&a.export(ExportMode::updates(&Default::default())).unwrap());
    let r3 = a.import(&b.export(ExportMode::updates(&Default::default())).unwrap());
    println!("B imports forged: {:?}, B imports A: {:?}, A imports B: {:?}", r1.is_ok(), r2.is_ok(), r3.is_ok());
    println!("A sees: {:?}", a.get_map("tasks").get_deep_value());
    println!("B sees: {:?}", b.get_map("tasks").get_deep_value());
    let who = b.get_map("tasks").get_last_editor("t1");
    println!("B's 'last editor' of t1 = {:?} (1 = A, the victim)", who);

    println!("\n## 2. Mass delete by a member");
    let a = LoroDoc::new(); a.set_peer_id(1).unwrap();
    let tasks = a.get_map("tasks");
    for i in 0..500 { tasks.insert(&format!("t{i}"), format!("task {i}")).unwrap(); }
    a.commit();
    let before = a.oplog_frontiers();
    let m = LoroDoc::from_snapshot(&a.export(ExportMode::Snapshot).unwrap()).unwrap(); m.set_peer_id(3).unwrap();
    let mt = m.get_map("tasks");
    for k in mt.keys().collect::<Vec<_>>() { mt.delete(&k).unwrap(); }
    m.commit();
    a.import(&m.export(ExportMode::updates(&a.oplog_vv())).unwrap()).unwrap();
    println!("after malicious delete: {} tasks", a.get_map("tasks").len());
    let restored = a.fork_at(&before).unwrap();
    println!("fork_at(before) still has {} tasks -> recoverable while history exists", restored.get_map("tasks").len());

    println!("\n## 3. Huge update (storage / bandwidth bomb)");
    let m = LoroDoc::new(); m.set_peer_id(9).unwrap();
    let big = "x".repeat(50 * 1024 * 1024);
    let t = Instant::now();
    m.get_text("desc").insert(0, &big).unwrap(); m.commit();
    let upd = m.export(ExportMode::updates(&Default::default())).unwrap();
    println!("50 MB text insert -> update of {} bytes (compressed), built in {:?}", upd.len(), t.elapsed());
    let victim = LoroDoc::new(); victim.set_peer_id(1).unwrap();
    let t = Instant::now();
    victim.import(&upd).unwrap();
    println!("victim import {:?}; victim now holds {} chars", t.elapsed(), victim.get_text("desc").len_unicode());
    let rnd: String = (0..5_000_000).map(|i| char::from(b'a' + ((i * 7919 + i / 13) % 26) as u8)).collect();
    let m2 = LoroDoc::new(); m2.set_peer_id(10).unwrap();
    m2.get_text("desc").insert(0, &rnd).unwrap(); m2.commit();
    println!("5 MB low-repetition text -> update of {} bytes", m2.export(ExportMode::updates(&Default::default())).unwrap().len());
}
