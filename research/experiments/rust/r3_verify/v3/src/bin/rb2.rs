use loro::{ExportMode, LoroDoc};
fn main() {
    let dir = std::env::args().nth(1).unwrap();
    for f in ["nc_resnapshot.bin", "nc_resnapshot_fromupd.bin", "nc_reupdates.bin"] {
        let b = std::fs::read(format!("{dir}/{f}")).unwrap();
        let d = LoroDoc::new();
        let r = d.import(&b);
        println!("{f}: import {:?}\n   value {:?}", r.map(|s| s.pending.is_some()), d.get_deep_value());
        // does a snapshot->fresh doc (state path) agree with an updates replay (op path)?
        let replay = LoroDoc::new();
        replay.import(&d.export(ExportMode::all_updates()).unwrap()).unwrap();
        println!("   state-path == op-replay: {}", replay.get_deep_value() == d.get_deep_value());
    }
}
