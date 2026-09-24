use loro::{ExportMode, LoroDoc, LoroMap, LoroValue, UndoManager, ValueOrContainer};

fn index_of(d: &LoroDoc, id: &str) -> Option<usize> {
    let l = d.get_movable_list("list");
    (0..l.len()).find(|&i| {
        matches!(l.get(i), Some(ValueOrContainer::Container(loro::Container::Map(m)))
            if matches!(m.get("id"), Some(ValueOrContainer::Value(LoroValue::String(s))) if &*s == id))
    })
}
fn map_at(d: &LoroDoc, idx: usize) -> LoroMap {
    match d.get_movable_list("list").get(idx) {
        Some(ValueOrContainer::Container(loro::Container::Map(m))) => m,
        _ => panic!("expected map container"),
    }
}

fn i1046() {
    let base = LoroDoc::new();
    base.set_peer_id(1).unwrap();
    let list = base.get_movable_list("list");
    for tag in ["a", "c"] {
        let m = list.insert_container(list.len(), LoroMap::new()).unwrap();
        m.insert("id", tag).unwrap();
    }
    base.commit();
    let snap = base.export(ExportMode::Snapshot).unwrap();
    let mk = |peer: u64| { let d = LoroDoc::new(); d.import(&snap).unwrap(); d.set_peer_id(peer).unwrap(); d.commit(); d };
    let pa = mk(0xA0);
    let pb = mk(0xA1);
    pa.get_movable_list("list").delete(index_of(&pa, "c").unwrap(), 1).unwrap();
    pa.commit();
    let v0 = pb.oplog_vv();
    map_at(&pb, index_of(&pb, "c").unwrap()).insert("contents", "zombie").unwrap();
    pb.commit();
    let b_edit = pb.export(ExportMode::updates(&v0)).unwrap();
    let v1 = pb.oplog_vv();
    pb.get_movable_list("list").mov(index_of(&pb, "c").unwrap(), 0).unwrap();
    pb.commit();
    let b_move = pb.export(ExportMode::updates(&v1)).unwrap();
    pa.import(&b_edit).unwrap();
    pa.import(&b_move).unwrap();
    pb.import(&pa.export(ExportMode::all_updates()).unwrap()).unwrap();
    println!("#1046: no panic; list len {}; converged={}", pa.get_movable_list("list").len(), pa.get_deep_value() == pb.get_deep_value());
}

fn i1055() {
    let a = LoroDoc::new();
    a.set_peer_id(1).unwrap();
    let tree = a.get_tree("tree");
    let node = tree.create(None).unwrap();
    a.commit();
    let mut um = UndoManager::new(&a);
    um.set_merge_interval(0);
    let body = tree.get_meta(node).unwrap().ensure_mergeable_text("body").unwrap();
    body.insert(0, "hello").unwrap();
    a.commit();
    let snapshot = a.export(ExportMode::Snapshot).unwrap();
    let fork_vv = a.oplog_vv();
    let b = LoroDoc::new();
    b.set_peer_id(2).unwrap();
    b.import(&snapshot).unwrap();
    let other = b.get_tree("tree").create(None).unwrap();
    b.get_tree("tree").get_meta(other).unwrap().ensure_mergeable_text("body").unwrap().insert(0, "unrelated").unwrap();
    b.commit();
    a.import(&b.export(ExportMode::updates(&fork_vv)).unwrap()).unwrap();
    let fresh = tree.get_meta(node).unwrap().ensure_mergeable_text("body").unwrap();
    fresh.insert(fresh.len_unicode(), " world").unwrap();
    a.commit();
    um.undo().unwrap();
    println!("#1055: original node deleted after undo = {:?}", tree.is_node_deleted(&node));
}

fn sizes() {
    // single-op update sizes and per-op snapshot sizes
    let d = LoroDoc::new(); d.set_peer_id(1).unwrap();
    let m = d.get_map("tasks");
    let mut tot = 0usize; let n = 1000;
    for i in 0..n {
        let vv = d.oplog_vv();
        m.insert(&format!("t{}", i % 50), i as i64).unwrap();
        d.commit();
        tot += d.export(ExportMode::updates(&vv)).unwrap().len();
    }
    println!("map set, same peer: avg single-op update {} B; snapshot {} B for {} ops ({:.1} B/op)", tot / n, d.export(ExportMode::Snapshot).unwrap().len(), n, d.export(ExportMode::Snapshot).unwrap().len() as f64 / n as f64);
    let d2 = LoroDoc::new();
    let mut tot2 = 0usize;
    for i in 0..n {
        let vv = d2.oplog_vv();
        d2.set_peer_id(1000 + i as u64).unwrap();
        d2.get_map("tasks").insert(&format!("t{}", i % 50), i as i64).unwrap();
        d2.commit();
        tot2 += d2.export(ExportMode::updates(&vv)).unwrap().len();
    }
    println!("map set, new peer each op: avg single-op update {} B; snapshot {} B ({:.1} B/op)", tot2 / n, d2.export(ExportMode::Snapshot).unwrap().len(), d2.export(ExportMode::Snapshot).unwrap().len() as f64 / n as f64);
    let d3 = LoroDoc::new(); d3.set_peer_id(1).unwrap();
    let t = d3.get_tree("t");
    let vv = d3.oplog_vv();
    let id = t.create(None).unwrap();
    t.get_meta(id).unwrap().insert("title", "Buy milk").unwrap();
    d3.commit();
    println!("tree create + title: update {} B", d3.export(ExportMode::updates(&vv)).unwrap().len());
}

fn main() {
    let which = std::env::args().nth(1).unwrap_or_default();
    match which.as_str() { "1046" => i1046(), "1055" => i1055(), "sizes" => sizes(), _ => {} }
}
