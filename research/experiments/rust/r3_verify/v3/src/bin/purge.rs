use loro::{ContainerTrait, ExportMode, LoroDoc};
fn main() {
    let n = 500;
    let body = "x".repeat(200);
    // A: plain tree meta, delete all nodes
    let d = LoroDoc::new(); d.set_peer_id(1).unwrap();
    let t = d.get_tree("t");
    let mut ids = vec![];
    for i in 0..n { let id = t.create(None).unwrap(); let m = t.get_meta(id).unwrap(); m.insert("title", format!("task {i}")).unwrap(); m.insert("desc", body.as_str()).unwrap(); ids.push(id); }
    d.commit();
    let before = d.export(ExportMode::StateOnly(None)).unwrap().len();
    for id in &ids { t.delete(*id).unwrap(); }
    d.commit();
    let f = d.oplog_frontiers();
    println!("tree+meta: state-only before delete {} B; after deleting all {} B; shallow@head {} B; live nodes {}", before, d.export(ExportMode::StateOnly(None)).unwrap().len(), d.export(ExportMode::shallow_snapshot(&f)).unwrap().len(), t.nodes().iter().filter(|x| !t.is_node_deleted(x).unwrap()).count());
    // B: mergeable text per node, delete node, then delete_root_container on the mergeable text
    let d2 = LoroDoc::new(); d2.set_peer_id(2).unwrap();
    let t2 = d2.get_tree("t");
    let mut cids = vec![]; let mut ids2 = vec![];
    for _ in 0..n { let id = t2.create(None).unwrap(); let txt = t2.get_meta(id).unwrap().ensure_mergeable_text("body").unwrap(); txt.insert(0, &body).unwrap(); cids.push(txt.id()); ids2.push(id); }
    d2.commit();
    println!("mergeable text container id example: {:?} is_root={}", cids[0], cids[0].is_root());
    for id in &ids2 { t2.delete(*id).unwrap(); }
    d2.commit();
    let s1 = d2.export(ExportMode::StateOnly(None)).unwrap().len();
    for c in &cids { d2.delete_root_container(c.clone()); }
    d2.commit();
    let s2 = d2.export(ExportMode::StateOnly(None)).unwrap().len();
    let f2 = d2.oplog_frontiers();
    let sh = d2.export(ExportMode::shallow_snapshot(&f2)).unwrap();
    let fresh = LoroDoc::from_snapshot(&sh).unwrap();
    let still = fresh.get_text(cids[0].clone()).to_string();
    println!("mergeable+delete_root_container: state-only after node delete {} B; after purge {} B; shallow {} B; purged text still readable in shallow import: {:?}", s1, s2, sh.len(), &still[..still.len().min(10)]);
    // C: mergeable text whose owner node is alive, purge works?
    let d3 = LoroDoc::new(); d3.set_peer_id(3).unwrap();
    let t3 = d3.get_tree("t");
    let id = t3.create(None).unwrap(); let txt = t3.get_meta(id).unwrap().ensure_mergeable_text("body").unwrap(); txt.insert(0, &body).unwrap(); d3.commit();
    d3.delete_root_container(txt.id()); d3.commit();
    println!("purge with live owner: text now {:?} len", d3.get_text(txt.id()).len_unicode());
}
