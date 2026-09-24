//! E1: Loro history growth, shallow snapshots, imports that depend on trimmed
//! history, and large-list performance.
use loro::{ExportMode, LoroDoc, TreeID};
use rand::{rngs::StdRng, seq::SliceRandom, Rng, SeedableRng};
use std::time::Instant;

const WORDS: &[&str] = &[
    "milk", "bread", "eggs", "apples", "coffee", "rice", "pasta", "cheese", "tomatoes", "onions",
    "soap", "batteries", "call plumber", "pay rent", "dentist", "birthday gift", "laundry",
];

fn kb(n: usize) -> String {
    format!("{:.1} KB", n as f64 / 1024.0)
}

fn add_task(doc: &LoroDoc, rng: &mut StdRng) -> TreeID {
    let tree = doc.get_tree("tasks");
    let id = tree.create(None).unwrap();
    let meta = tree.get_meta(id).unwrap();
    let title = format!("{} x{}", WORDS.choose(rng).unwrap(), rng.gen_range(1..5));
    meta.insert("title", title).unwrap();
    meta.insert("done", false).unwrap();
    id
}

/// Shared shopping list used daily: 3 adds, 3 check-offs, 3 deletes, 1 move.
fn daily_shopping(doc: &LoroDoc, live: &mut Vec<TreeID>, rng: &mut StdRng) {
    let tree = doc.get_tree("tasks");
    for _ in 0..3 {
        live.push(add_task(doc, rng));
    }
    for _ in 0..3 {
        if let Some(id) = live.choose(rng).copied() {
            tree.get_meta(id).unwrap().insert("done", true).unwrap();
        }
    }
    for _ in 0..3 {
        if live.len() > 20 {
            let i = rng.gen_range(0..live.len());
            let id = live.swap_remove(i);
            tree.delete(id).unwrap();
        }
    }
    if live.len() > 2 {
        let a = live[rng.gen_range(0..live.len())];
        let b = live[rng.gen_range(0..live.len())];
        if a != b {
            let _ = tree.mov(a, b); // make a subtask of b
            let _ = tree.mov(a, None); // and back
        }
    }
    doc.commit();
}

fn experiment_growth() {
    println!("\n## 1. History growth: shared shopping list, 10 edits/day, 2 devices alternating");
    let mut rng = StdRng::seed_from_u64(1);
    let a = LoroDoc::new();
    a.set_peer_id(1).unwrap();
    let tree = a.get_tree("tasks");
    tree.enable_fractional_index(0);
    let b = LoroDoc::new();
    b.set_peer_id(2).unwrap();
    let mut live_a = Vec::new();
    let mut live_b = Vec::new();
    println!("| Years | Full snapshot (with history) | State-only | Ops |");
    println!("|---|---|---|---|");
    for day in 1..=(365 * 5) {
        if day % 2 == 0 {
            daily_shopping(&a, &mut live_a, &mut rng);
            b.import(&a.export(ExportMode::updates(&b.oplog_vv())).unwrap()).unwrap();
            live_b = live_a.clone();
        } else {
            daily_shopping(&b, &mut live_b, &mut rng);
            a.import(&b.export(ExportMode::updates(&a.oplog_vv())).unwrap()).unwrap();
            live_a = live_b.clone();
        }
        if day % 365 == 0 {
            let full = a.export(ExportMode::Snapshot).unwrap();
            let state = a.export(ExportMode::StateOnly(None)).unwrap();
            println!("| {} | {} | {} | {} |", day / 365, kb(full.len()), kb(state.len()), a.len_ops());
        }
    }
}

fn experiment_typing() {
    println!("\n## 2. Character-by-character typing in a task description");
    let doc = LoroDoc::new();
    doc.set_peer_id(1).unwrap();
    let text = doc.get_text("desc");
    let sample = "Remember to bring the blue folder and ask about the invoice from March. ";
    let mut typed = 0usize;
    for round in 0..200 {
        for ch in sample.chars() {
            let len = text.len_unicode();
            text.insert(len, &ch.to_string()).unwrap();
            typed += 1;
            doc.commit(); // worst case: every keystroke committed separately
        }
        // delete a word occasionally (editing)
        if round % 5 == 0 && text.len_unicode() > 10 {
            text.delete(0, 10).unwrap();
            doc.commit();
        }
    }
    let full = doc.export(ExportMode::Snapshot).unwrap();
    let state = doc.export(ExportMode::StateOnly(None)).unwrap();
    println!(
        "{} keystrokes, final text {} chars: full snapshot {}, state-only {} (history/state ratio {:.1}x)",
        typed,
        text.len_unicode(),
        kb(full.len()),
        kb(state.len()),
        full.len() as f64 / state.len() as f64
    );
}

fn experiment_trimmed_history() {
    println!("\n## 3. Importing updates that depend on trimmed history");
    let mut rng = StdRng::seed_from_u64(2);
    let a = LoroDoc::new();
    a.set_peer_id(1).unwrap();
    let mut live = Vec::new();
    for _ in 0..50 {
        daily_shopping(&a, &mut live, &mut rng);
    }
    // B syncs at this point, then goes offline and edits.
    let b = LoroDoc::from_snapshot(&a.export(ExportMode::Snapshot).unwrap()).unwrap();
    b.set_peer_id(2).unwrap();
    let b_version_before = b.oplog_vv();
    let mut live_b = live.clone();
    for _ in 0..5 {
        daily_shopping(&b, &mut live_b, &mut rng);
    }
    // Meanwhile A keeps going and compacts history to "now".
    for _ in 0..50 {
        daily_shopping(&a, &mut live, &mut rng);
    }
    let shallow = a.export(ExportMode::shallow_snapshot(&a.oplog_frontiers())).unwrap();
    let a2 = LoroDoc::from_snapshot(&shallow).unwrap();
    a2.set_peer_id(3).unwrap();
    println!(
        "A full snapshot {} vs shallow snapshot {}",
        kb(a.export(ExportMode::Snapshot).unwrap().len()),
        kb(shallow.len())
    );
    // B returns and sends only its own new ops since it went offline.
    let b_updates = b.export(ExportMode::updates(&b_version_before)).unwrap();
    match a2.import(&b_updates) {
        Ok(status) => println!("Import into shallow doc: OK. pending = {:?}", status.pending.is_some()),
        Err(e) => println!("Import into shallow doc: ERROR: {e}"),
    }
    // What about the reverse: B pulls the shallow snapshot?
    match b.import(&shallow) {
        Ok(_) => println!("B importing A's shallow snapshot: OK"),
        Err(e) => println!("B importing A's shallow snapshot: ERROR: {e}"),
    }
    // Rebase path: B re-applies its unsynced changes on top of the shallow doc as new ops.
    let rebased = LoroDoc::from_snapshot(&shallow).unwrap();
    rebased.set_peer_id(4).unwrap();
    let before = rebased.get_tree("tasks").nodes().len();
    let mut live_r = Vec::new();
    for _ in 0..5 {
        daily_shopping(&rebased, &mut live_r, &mut rng);
    }
    println!(
        "Rebase path (re-applying as new ops on the shallow doc): OK, nodes {} -> {}",
        before,
        rebased.get_tree("tasks").nodes().len()
    );
}

fn experiment_checkpoints() {
    println!("\n## 4. State checkpoints at old versions (history thinning)");
    let mut rng = StdRng::seed_from_u64(3);
    let doc = LoroDoc::new();
    doc.set_peer_id(1).unwrap();
    let mut live = Vec::new();
    let mut frontiers = Vec::new();
    for day in 0..365 {
        daily_shopping(&doc, &mut live, &mut rng);
        if day % 30 == 0 {
            frontiers.push(doc.oplog_frontiers());
        }
    }
    let mut total = 0;
    for f in &frontiers {
        total += doc.export(ExportMode::StateOnly(Some(std::borrow::Cow::Borrowed(f)))).unwrap().len();
    }
    let forked = doc.fork_at(&frontiers[3]).unwrap();
    println!(
        "{} monthly state checkpoints: {} total; fork_at works (nodes at month 3: {}, now: {})",
        frontiers.len(),
        kb(total),
        forked.get_tree("tasks").nodes().len(),
        doc.get_tree("tasks").nodes().len()
    );
}

fn experiment_large_list() {
    println!("\n## 5. Large list performance");
    let mut rng = StdRng::seed_from_u64(4);
    let doc = LoroDoc::new();
    doc.set_peer_id(1).unwrap();
    let t = Instant::now();
    for _ in 0..10_000 {
        add_task(&doc, &mut rng);
    }
    doc.commit();
    let create = t.elapsed();
    let t = Instant::now();
    let snap = doc.export(ExportMode::Snapshot).unwrap();
    let export = t.elapsed();
    let t = Instant::now();
    let loaded = LoroDoc::from_snapshot(&snap).unwrap();
    let _ = loaded.get_tree("tasks").nodes().len();
    let import = t.elapsed();
    println!(
        "10,000 tasks: create {:?}, snapshot {} in {:?}, load {:?}",
        create,
        kb(snap.len()),
        export,
        import
    );
}

fn experiment_peer_id_reuse() {
    println!("\n## 6. Two devices accidentally sharing a PeerID");
    let a = LoroDoc::new();
    a.set_peer_id(7).unwrap();
    let b = LoroDoc::new();
    b.set_peer_id(7).unwrap();
    a.get_map("m").insert("x", "from A").unwrap();
    a.commit();
    b.get_map("m").insert("x", "from B").unwrap();
    b.commit();
    let r1 = a.import(&b.export(ExportMode::Snapshot).unwrap());
    let r2 = b.import(&a.export(ExportMode::Snapshot).unwrap());
    println!(
        "A imports B: {:?}; B imports A: {:?}; A sees {:?}, B sees {:?}",
        r1.map(|s| s.pending.is_some()),
        r2.map(|s| s.pending.is_some()),
        a.get_map("m").get("x"),
        b.get_map("m").get("x")
    );
}

fn main() {
    println!("# E1 results (loro {})", env!("CARGO_PKG_VERSION"));
    experiment_growth();
    experiment_typing();
    experiment_trimmed_history();
    experiment_checkpoints();
    experiment_large_list();
    experiment_peer_id_reuse();
}
