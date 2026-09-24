//! E1b: tombstone growth by data model, and realistic (non-repetitive) typing.
use loro::{ExportMode, LoroDoc, LoroMap};
use rand::{rngs::StdRng, Rng, SeedableRng, distributions::Alphanumeric};

fn kb(n: usize) -> String { format!("{:.1} KB", n as f64 / 1024.0) }
fn rtext(rng: &mut StdRng, n: usize) -> String { (0..n).map(|_| rng.sample(Alphanumeric) as char).collect() }

fn main() {
    // Same churn for 3 models: keep ~20 live items, add 5 + delete 5 per day, 2 years.
    let days = 730;
    println!("| Model | live items | State-only | Full snapshot |\n|---|---|---|---|");

    // A) LoroTree with node meta
    let mut rng = StdRng::seed_from_u64(9);
    let doc = LoroDoc::new(); doc.set_peer_id(1).unwrap();
    let tree = doc.get_tree("t"); tree.enable_fractional_index(0);
    let mut live = Vec::new();
    for _ in 0..days { for _ in 0..5 { let id = tree.create(None).unwrap(); tree.get_meta(id).unwrap().insert("title", rtext(&mut rng, 16)).unwrap(); live.push(id); }
        while live.len() > 20 { let id = live.remove(0); tree.delete(id).unwrap(); } doc.commit(); }
    println!("| LoroTree | {} | {} | {} |", live.len(), kb(doc.export(ExportMode::StateOnly(None)).unwrap().len()), kb(doc.export(ExportMode::Snapshot).unwrap().len()));

    // B) LoroMap id -> LoroMap {title, parent, order}
    let mut rng = StdRng::seed_from_u64(9);
    let doc = LoroDoc::new(); doc.set_peer_id(1).unwrap();
    let tasks = doc.get_map("tasks");
    let mut live: Vec<String> = Vec::new(); let mut n = 0u64;
    for _ in 0..days { for _ in 0..5 { n += 1; let k = format!("{n:x}"); let m = tasks.insert_container(&k, LoroMap::new()).unwrap(); m.insert("title", rtext(&mut rng, 16)).unwrap(); m.insert("order", n as f64).unwrap(); live.push(k); }
        while live.len() > 20 { let k = live.remove(0); tasks.delete(&k).unwrap(); } doc.commit(); }
    println!("| LoroMap of maps | {} | {} | {} |", live.len(), kb(doc.export(ExportMode::StateOnly(None)).unwrap().len()), kb(doc.export(ExportMode::Snapshot).unwrap().len()));

    // C) LoroMovableList of maps
    let mut rng = StdRng::seed_from_u64(9);
    let doc = LoroDoc::new(); doc.set_peer_id(1).unwrap();
    let list = doc.get_movable_list("tasks");
    for _ in 0..days { for _ in 0..5 { let m = list.push_container(LoroMap::new()).unwrap(); m.insert("title", rtext(&mut rng, 16)).unwrap(); }
        while list.len() > 20 { list.delete(0, 1).unwrap(); } doc.commit(); }
    println!("| LoroMovableList of maps | {} | {} | {} |", list.len(), kb(doc.export(ExportMode::StateOnly(None)).unwrap().len()), kb(doc.export(ExportMode::Snapshot).unwrap().len()));

    // D) Fresh-document compaction: copy only live state into a new doc
    let fresh = LoroDoc::new(); fresh.set_peer_id(2).unwrap();
    let ft = fresh.get_map("tasks");
    for i in 0..20 { let m = ft.insert_container(&format!("{i}"), LoroMap::new()).unwrap(); m.insert("title", rtext(&mut rng, 16)).unwrap(); }
    fresh.commit();
    println!("| Fresh doc (live state re-created) | 20 | {} | {} |", kb(fresh.export(ExportMode::StateOnly(None)).unwrap().len()), kb(fresh.export(ExportMode::Snapshot).unwrap().len()));

    // Realistic typing: random words, keystroke commits
    let doc = LoroDoc::new(); doc.set_peer_id(1).unwrap();
    let text = doc.get_text("d"); let mut rng = StdRng::seed_from_u64(5);
    let mut ks = 0;
    for _ in 0..2000 { let wl = rng.gen_range(3..9); let w = rtext(&mut rng, wl) + " ";
        for ch in w.chars() { let l = text.len_unicode(); text.insert(l, &ch.to_string()).unwrap(); doc.commit(); ks += 1; }
        if rng.gen_bool(0.1) { let l = text.len_unicode(); if l > 5 { text.delete(l - 5, 5).unwrap(); doc.commit(); } } }
    let s = doc.export(ExportMode::StateOnly(None)).unwrap().len(); let f = doc.export(ExportMode::Snapshot).unwrap().len();
    println!("\nRandom typing: {ks} keystrokes, {} chars final: state {} full {} (ratio {:.1}x)", text.len_unicode(), kb(s), kb(f), f as f64 / s as f64);
}
