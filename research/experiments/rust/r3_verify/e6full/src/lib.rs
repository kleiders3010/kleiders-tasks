//! Broader Loro API surface than the E6 probe, to bracket native size.
use loro::{ExportMode, LoroDoc, LoroMap, LoroText, UndoManager, ExpandType, StyleConfig, StyleConfigMap};
use std::ffi::c_int;

#[no_mangle]
pub extern "C" fn probe_full(n: c_int, buf: *const u8, len: usize) -> c_int {
    let input = unsafe { std::slice::from_raw_parts(buf, len) };
    let doc = LoroDoc::new();
    let mut styles = StyleConfigMap::new();
    styles.insert("bold".into(), StyleConfig { expand: ExpandType::After });
    doc.config_text_style(styles);
    doc.set_record_timestamp(true);
    let _s = doc.subscribe_root(std::sync::Arc::new(|e| { let _ = e.events.len(); }));
    let mut um = UndoManager::new(&doc);
    let tree = doc.get_tree("t");
    tree.enable_fractional_index(0);
    for i in 0..n {
        let id = tree.create(None).unwrap();
        let meta = tree.get_meta(id).unwrap();
        meta.insert("title", i as i64).unwrap();
        let t = meta.insert_container("desc", LoroText::new()).unwrap();
        t.insert(0, "hello world").unwrap();
        t.mark(0..5, "bold", true).unwrap();
        let _ = meta.get_or_create_container("sub", LoroMap::new());
        let _ = t.to_delta();
    }
    doc.get_counter("c").increment(1.0).unwrap();
    doc.get_movable_list("l").push(1).unwrap();
    doc.get_list("ll").push("x").unwrap();
    doc.commit();
    um.undo().ok();
    um.redo().ok();
    let f = doc.oplog_frontiers();
    let _ = doc.checkout(&f);
    doc.checkout_to_latest();
    let js = doc.export_json_updates(&Default::default(), &doc.oplog_vv());
    let d3 = LoroDoc::new();
    let _ = d3.import_json_updates(js);
    let forked = doc.fork_at(&f).unwrap();
    let snap = doc.export(ExportMode::Snapshot).unwrap();
    let sh = doc.export(ExportMode::shallow_snapshot(&f)).unwrap();
    let so = doc.export(ExportMode::StateOnly(None)).unwrap();
    let d2 = LoroDoc::from_snapshot(&snap).unwrap();
    let _ = d2.import(input);
    let _ = d2.import_batch(&[sh.clone(), so.clone()]);
    let v = d2.get_deep_value();
    let s = format!("{:?}", v);
    let _ = doc.diff(&Default::default(), &f);
    (snap.len() + sh.len() + so.len() + s.len() + forked.oplog_vv().len()) as c_int
}
