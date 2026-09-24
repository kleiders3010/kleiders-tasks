//! Abort-path repros on loro 1.16.2: #1068 and PR #1083 case 2.
use loro::{ExportMode, IdSpan, LoroDoc};

fn setup() -> (LoroDoc, Vec<u8>, Vec<u8>, Vec<u8>) {
    let b = LoroDoc::new();
    b.set_peer_id(2).unwrap();
    b.get_map("m").insert("k0", "v0").unwrap();
    b.commit();
    let b0 = b.export(ExportMode::all_updates()).unwrap();
    let a = LoroDoc::new();
    a.set_peer_id(1).unwrap();
    a.import(&b0).unwrap();
    a.get_map("m").insert("k1", "v1").unwrap();
    a.commit();
    let snapshot = a.export(ExportMode::shallow_snapshot(&a.oplog_frontiers())).unwrap();
    b.get_map("m").insert("k2", "v2").unwrap();
    b.commit();
    let b1 = b.export(ExportMode::updates_in_range(vec![IdSpan::new(2, 1, 2)])).unwrap();
    (b, b0, snapshot, b1)
}

fn main() {
    let case = std::env::args().nth(1).unwrap_or_default();
    let (_b, b0, snapshot, b1) = setup();
    match case.as_str() {
        "1068" => {
            let fresh = LoroDoc::new();
            fresh.import(&snapshot).unwrap();
            fresh.set_peer_id(9).unwrap();
            let r = fresh.import(&b1);
            println!("1068: import returned {r:?}; value {:?}", fresh.get_deep_value());
        }
        "1083b" => {
            // parked before shallow import, then unlocked
            let x = LoroDoc::new();
            x.set_peer_id(9).unwrap();
            println!("park b1 -> {:?}", x.import(&b1));
            println!("import shallow -> {:?}", x.import(&snapshot));
            println!("value after snapshot {:?}", x.get_deep_value());
            println!("import b0 (unlock) -> {:?}", x.import(&b0));
            println!("value {:?}", x.get_deep_value());
        }
        "1083c" => {
            // peer 3 syncs b0 and writes P (deps [2@0]); X parks P, imports A's shallow
            // snapshot (root trims 2@0), then imports a valid post-root change from peer 2.
            let c = LoroDoc::new();
            c.set_peer_id(3).unwrap();
            c.import(&b0).unwrap();
            c.get_map("m").insert("p", "from peer 3").unwrap();
            c.commit();
            let p = c.export(ExportMode::updates_in_range(vec![IdSpan::new(3, 0, 1)])).unwrap();
            let x = LoroDoc::new();
            x.set_peer_id(9).unwrap();
            println!("park P -> {:?}", x.import(&p));
            println!("import shallow -> {:?}", x.import(&snapshot));
            // B catches up on A post-root and writes a valid post-root op
            let b = LoroDoc::new();
            b.set_peer_id(2).unwrap();
            b.import(&snapshot).unwrap();
            b.get_map("m").insert("post", "root").unwrap();
            b.commit();
            let post = b.export(ExportMode::updates_in_range(vec![IdSpan::new(2, 1, 2)])).unwrap();
            let _ = post;
            // B in this construction is a fresh doc with peer 2 continuing at counter 1
            println!("B frontiers {:?}", b.oplog_frontiers());
            let r = x.import(&b.export(ExportMode::updates(&x.oplog_vv())).unwrap());
            println!("import valid post-root change from peer 2 -> {r:?}");
            println!("value {:?}", x.get_deep_value());
        }
        "1083d" | "1083e" => {
            let c = LoroDoc::new();
            c.set_peer_id(3).unwrap();
            c.import(&b0).unwrap();
            c.get_map("m").insert("p", "from peer 3").unwrap();
            c.commit();
            let p = c.export(ExportMode::updates_in_range(vec![IdSpan::new(3, 0, 1)])).unwrap();
            let x = LoroDoc::new();
            x.set_peer_id(9).unwrap();
            x.import(&p).unwrap();
            x.import(&snapshot).unwrap();
            let b = LoroDoc::new();
            b.set_peer_id(2).unwrap();
            b.import(&snapshot).unwrap();
            b.get_map("m").insert("post", "root").unwrap();
            b.commit();
            let blob = b.export(ExportMode::updates(&x.oplog_vv())).unwrap();
            std::panic::set_hook(Box::new(|i| eprintln!("[panic hook] {}", i)));
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| x.import(&blob)));
            println!("caught panic: {}", r.is_err());
            if case == "1083d" {
                println!("leaking poisoned doc with mem::forget");
                std::mem::forget(x);
            } else {
                println!("dropping poisoned doc normally");
                let r2 = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || drop(x)));
                println!("drop panicked: {}", r2.is_err());
            }
            println!("process still alive");
        }
        _ => println!("usage: 1068 | 1083b | 1083c | 1083d | 1083e"),
    }
}
