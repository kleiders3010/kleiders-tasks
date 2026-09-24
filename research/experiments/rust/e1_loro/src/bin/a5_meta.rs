use loro::{ExportMode, LoroDoc};
fn main() {
    let a = LoroDoc::new(); a.set_peer_id(1).unwrap();
    let b = LoroDoc::new(); b.set_peer_id(2).unwrap();
    a.get_map("m").insert("x", 1).unwrap(); a.commit();
    b.import(&a.export(ExportMode::Snapshot).unwrap()).unwrap();
    let vv_before = b.oplog_vv();
    b.get_map("m").insert("y", 2).unwrap(); b.commit();
    let only_b = b.export(ExportMode::updates(&vv_before)).unwrap();
    let all = b.export(ExportMode::updates(&Default::default())).unwrap();
    let snap = b.export(ExportMode::Snapshot).unwrap();
    for (name, blob) in [("B's own updates", &only_b), ("all updates", &all), ("snapshot", &snap)] {
        let meta = LoroDoc::decode_import_blob_meta(blob, true).unwrap();
        println!("{name}: mode={:?} start_vv={:?} end_vv={:?} changes={}", meta.mode, meta.partial_start_vv, meta.partial_end_vv, meta.change_num);
    }
}
