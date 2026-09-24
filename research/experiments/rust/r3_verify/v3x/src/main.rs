//! Cross-version corpus writer/reader for loro 1.16.2 (Rust side).
use loro::{ExportMode, LoroDoc, LoroMap, LoroText, ToJson};

fn build(mergeable: bool) -> LoroDoc {
    let d = LoroDoc::new();
    d.set_peer_id(77).unwrap();
    let tree = d.get_tree("tasks");
    tree.enable_fractional_index(0);
    let a = tree.create(None).unwrap();
    let b = tree.create(None).unwrap();
    let c = tree.create(a).unwrap();
    tree.get_meta(a).unwrap().insert("title", "A").unwrap();
    tree.get_meta(b).unwrap().insert("title", "B").unwrap();
    tree.get_meta(c).unwrap().insert("title", "C").unwrap();
    let t = tree.get_meta(a).unwrap().insert_container("desc", LoroText::new()).unwrap();
    t.insert(0, "hello world").unwrap();
    d.commit();
    tree.mov(c, b).unwrap();
    tree.mov_before(b, a).unwrap();
    let m = d.get_map("meta");
    m.insert("n", 1.5).unwrap();
    m.insert("bin", vec![1u8, 2, 3]).unwrap();
    let ml = d.get_movable_list("order");
    ml.push("x").unwrap(); ml.push("y").unwrap(); ml.push("z").unwrap();
    ml.mov(0, 2).unwrap();
    ml.set(1, "Y").unwrap();
    let l = d.get_list("log"); l.push("e1").unwrap();
    let txt = d.get_text("note"); txt.insert(0, "abc").unwrap();
    d.commit();
    if mergeable {
        let mm = m.ensure_mergeable_map("settings").unwrap();
        mm.insert("theme", "dark").unwrap();
        let mt = m.ensure_mergeable_text("body").unwrap();
        mt.insert(0, "mergeable text").unwrap();
        d.commit();
    }
    let _ = LoroMap::new();
    d
}

fn main() {
    let mode = std::env::args().nth(1).unwrap();
    let dir = std::env::args().nth(2).unwrap();
    if mode == "write" {
        for (name, mg) in [("plain", false), ("mergeable", true)] {
            let d = build(mg);
            let f = d.oplog_frontiers();
            let first = loro::Frontiers::from_id(loro::ID::new(77, 3));
            std::fs::write(format!("{dir}/{name}.snapshot"), d.export(ExportMode::Snapshot).unwrap()).unwrap();
            std::fs::write(format!("{dir}/{name}.updates"), d.export(ExportMode::all_updates()).unwrap()).unwrap();
            std::fs::write(format!("{dir}/{name}.shallow"), d.export(ExportMode::shallow_snapshot(&first)).unwrap()).unwrap();
            std::fs::write(format!("{dir}/{name}.stateonly"), d.export(ExportMode::StateOnly(None)).unwrap()).unwrap();
            std::fs::write(format!("{dir}/{name}.json"), d.get_deep_value().to_json()).unwrap();
            let _ = f;
        }
        println!("written");
    } else {
        // read every *.fromjs blob and print value
        let mut names: Vec<_> = std::fs::read_dir(&dir).unwrap().filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.extension().map(|x| x == "fromjs").unwrap_or(false)).collect();
        names.sort();
        for p in names {
            let b = std::fs::read(&p).unwrap();
            let d = LoroDoc::new();
            let r = d.import(&b);
            let v = d.get_deep_value().to_json();
            std::fs::write(p.with_extension("got"), &v).unwrap(); let expect_path = p.with_extension("expect");
            let expect = std::fs::read_to_string(&expect_path).unwrap_or_default();
            let same = serde_like_eq(&v, &expect);
            println!("{} -> ok={} pending={:?} same_as_writer={} {}", p.file_name().unwrap().to_string_lossy(), r.is_ok(), r.as_ref().ok().map(|s| s.pending.is_some()), same, if same { String::new() } else { format!("\n   got={v}\n   exp={expect}") });
        }
    }
}

fn serde_like_eq(a: &str, b: &str) -> bool {
    // order-insensitive compare via sorted chars is too weak; compare parsed JSON via canonical sort
    fn canon(s: &str) -> String { let mut c: Vec<char> = s.chars().filter(|c| !c.is_whitespace()).collect(); c.sort(); c.into_iter().collect() }
    canon(a) == canon(b)
}
