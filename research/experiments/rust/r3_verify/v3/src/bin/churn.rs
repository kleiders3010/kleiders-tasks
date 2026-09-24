//! Replicates E1b churn (keep ~20 live, add 5 + delete 5 per day, 2 years) and adds
//! shallow-snapshot-at-head sizes, which E1 did not measure.
use loro::{ExportMode, LoroDoc, LoroMap};
fn kb(n: usize) -> String { format!("{:.1} KB", n as f64 / 1024.0) }
fn rtext(seed: &mut u64, n: usize) -> String { (0..n).map(|_| { *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407); (b'a' + ((*seed >> 33) % 26) as u8) as char }).collect() }
fn report(name: &str, d: &LoroDoc) {
    let f = d.oplog_frontiers();
    let sh = d.export(ExportMode::shallow_snapshot(&f)).unwrap();
    let re = LoroDoc::from_snapshot(&sh).unwrap();
    println!("| {name} | state-only {} | full {} | shallow@head {} | shallow value == live value: {} |", kb(d.export(ExportMode::StateOnly(None)).unwrap().len()), kb(d.export(ExportMode::Snapshot).unwrap().len()), kb(sh.len()), re.get_deep_value() == d.get_deep_value());
}
fn main() {
    let days = 730;
    let mut seed = 9u64;
    let doc = LoroDoc::new(); doc.set_peer_id(1).unwrap();
    let tree = doc.get_tree("t"); tree.enable_fractional_index(0);
    let mut live = Vec::new();
    for _ in 0..days { for _ in 0..5 { let id = tree.create(None).unwrap(); tree.get_meta(id).unwrap().insert("title", rtext(&mut seed, 16)).unwrap(); live.push(id); }
        while live.len() > 20 { let id = live.remove(0); tree.delete(id).unwrap(); } doc.commit(); }
    report("LoroTree", &doc);
    let doc = LoroDoc::new(); doc.set_peer_id(1).unwrap();
    let tasks = doc.get_map("tasks");
    let mut live: Vec<String> = Vec::new(); let mut n = 0u64;
    for _ in 0..days { for _ in 0..5 { n += 1; let k = format!("{n:x}"); let m = tasks.insert_container(&k, LoroMap::new()).unwrap(); m.insert("title", rtext(&mut seed, 16)).unwrap(); m.insert("order", n as f64).unwrap(); live.push(k); }
        while live.len() > 20 { let k = live.remove(0); tasks.delete(&k).unwrap(); } doc.commit(); }
    report("LoroMap of maps", &doc);
    let doc = LoroDoc::new(); doc.set_peer_id(1).unwrap();
    let list = doc.get_movable_list("tasks");
    for _ in 0..days { for _ in 0..5 { let m = list.push_container(LoroMap::new()).unwrap(); m.insert("title", rtext(&mut seed, 16)).unwrap(); }
        while list.len() > 20 { list.delete(0, 1).unwrap(); } doc.commit(); }
    report("LoroMovableList of maps", &doc);
    let doc = LoroDoc::new(); doc.set_peer_id(1).unwrap();
    let tree = doc.get_tree("t"); tree.enable_fractional_index(0);
    let mut live = Vec::new();
    for _ in 0..days { for _ in 0..5 { let id = tree.create(None).unwrap(); tree.get_meta(id).unwrap().ensure_mergeable_text("body").unwrap().insert(0, &rtext(&mut seed, 16)).unwrap(); live.push(id); }
        while live.len() > 20 { let id = live.remove(0); tree.delete(id).unwrap(); } doc.commit(); }
    report("LoroTree + mergeable text per node", &doc);
}
