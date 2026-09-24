//! Loro 1.16.2 built WITHOUT the `counter` feature: acts as an "old client" that
//! does not know the Counter container type.
use loro::{ExportMode, LoroDoc};

fn main() {
    let dir = std::env::args().nth(1).unwrap_or(".".into());
    let snap = std::fs::read(format!("{dir}/counter_snapshot.bin")).unwrap();
    let upd = std::fs::read(format!("{dir}/counter_updates.bin")).unwrap();

    println!("== NC1 import snapshot containing counters");
    let d = LoroDoc::new();
    d.set_peer_id(50).unwrap();
    println!("import snapshot -> {:?}", d.import(&snap));
    println!("deep value: {:?}", d.get_deep_value());

    println!("== NC2 import updates containing counters");
    let u = LoroDoc::new();
    u.set_peer_id(51).unwrap();
    println!("import updates -> {:?}", u.import(&upd));
    println!("deep value: {:?}", u.get_deep_value());

    // old client keeps editing known containers
    d.get_map("m").insert("edited_by_old", true).unwrap();
    d.commit();

    println!("== NC3 exports from the old client");
    let full = d.export(ExportMode::Snapshot);
    println!("Snapshot -> ok={} {:?}", full.is_ok(), full.as_ref().err());
    let so = d.export(ExportMode::StateOnly(None));
    println!("StateOnly -> ok={} {:?}", so.is_ok(), so.as_ref().err());
    let f = d.oplog_frontiers();
    let sh = d.export(ExportMode::shallow_snapshot(&f));
    println!("ShallowSnapshot(head) -> ok={} {:?}", sh.is_ok(), sh.as_ref().err());
    let up = d.export(ExportMode::all_updates());
    println!("Updates -> ok={} {:?}", up.is_ok(), up.as_ref().err());
    if let Ok(b) = full {
        std::fs::write(format!("{dir}/nc_resnapshot.bin"), b).unwrap();
    }
    if let Ok(b) = up {
        std::fs::write(format!("{dir}/nc_reupdates.bin"), b).unwrap();
    }

    println!("== NC4 same, but for a doc built from updates (no materialized state section)");
    let full_u = u.export(ExportMode::Snapshot);
    println!("Snapshot(from-updates doc) -> ok={} {:?}", full_u.is_ok(), full_u.as_ref().err());
    let so_u = u.export(ExportMode::StateOnly(None));
    println!("StateOnly(from-updates doc) -> ok={} {:?}", so_u.is_ok(), so_u.as_ref().err());
    if let Ok(b) = full_u {
        std::fs::write(format!("{dir}/nc_resnapshot_fromupd.bin"), b).unwrap();
    }
}
