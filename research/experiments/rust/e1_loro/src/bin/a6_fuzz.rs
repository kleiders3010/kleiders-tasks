//! A6: mutation fuzzing of Loro import. A malicious group member can send any
//! bytes (they hold the group key, so decryption succeeds). Import must return
//! Err, never panic/abort/hang or explode memory.
use loro::{ExportMode, LoroDoc};
use rand::{rngs::StdRng, Rng, SeedableRng};
use std::panic;
use std::time::{Duration, Instant};

fn seed_corpus() -> Vec<Vec<u8>> {
    let d = LoroDoc::new();
    d.set_peer_id(1).unwrap();
    let t = d.get_tree("t");
    for i in 0..30 { let id = t.create(None).unwrap(); t.get_meta(id).unwrap().insert("title", format!("task {i}")).unwrap(); }
    d.get_text("x").insert(0, "hello world").unwrap();
    d.get_movable_list("l").push(1).unwrap();
    d.commit();
    let v0 = d.oplog_vv();
    d.get_map("m").insert("k", "v").unwrap();
    d.commit();
    vec![
        d.export(ExportMode::Snapshot).unwrap(),
        d.export(ExportMode::updates(&Default::default())).unwrap(),
        d.export(ExportMode::updates(&v0)).unwrap(),
        d.export(ExportMode::StateOnly(None)).unwrap(),
        d.export(ExportMode::shallow_snapshot(&d.oplog_frontiers())).unwrap(),
    ]
}

fn mutate(rng: &mut StdRng, src: &[u8]) -> Vec<u8> {
    let mut v = src.to_vec();
    let n = rng.gen_range(1..8);
    for _ in 0..n {
        if v.is_empty() { v.push(rng.r#gen()); continue; }
        match rng.gen_range(0..6) {
            0 => { let i = rng.gen_range(0..v.len()); v[i] ^= 1 << rng.gen_range(0..8); }
            1 => { let i = rng.gen_range(0..v.len()); v[i] = rng.r#gen(); }
            2 => { let i = rng.gen_range(0..v.len()); v.truncate(i); }
            3 => { let i = rng.gen_range(0..v.len()); let b = [0xffu8, 0x7f, 0x80, 0x00][rng.gen_range(0..4)]; for j in i..(i + 4).min(v.len()) { v[j] = b; } }
            4 => { let i = rng.gen_range(0..v.len()); let len = rng.gen_range(1..16).min(v.len() - i); let chunk = v[i..i + len].to_vec(); let at = rng.gen_range(0..v.len()); v.splice(at..at, chunk); }
            _ => { let at = rng.gen_range(0..=v.len()); v.insert(at, rng.r#gen()); }
        }
    }
    v
}

fn main() {
    let secs: u64 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(120);
    let corpus = seed_corpus();
    let seed: u64 = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(0xC0FFEE); let mut rng = StdRng::seed_from_u64(seed);
    panic::set_hook(Box::new(|_| {}));
    let (mut runs, mut ok, mut err, mut panics, mut slow) = (0u64, 0u64, 0u64, 0u64, 0u64);
    let mut worst = Duration::ZERO;
    let mut panic_samples: Vec<(usize, String)> = Vec::new();
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(secs) {
        let base = &corpus[rng.gen_range(0..corpus.len())];
        // Mutate only the body (after magic + checksum + 2 mode bytes), then
        // recompute the checksum like a malicious member could.
        let mut input = base[..22].to_vec();
        input.extend(mutate(&mut rng, &base[22..]));
        let sum = xxhash_rust::xxh32::xxh32(&input[20..], u32::from_le_bytes(*b"LORO"));
        input[16..20].copy_from_slice(&sum.to_le_bytes());
        std::fs::write("last_input.bin", &input).ok();
        let t = Instant::now();
        let r = panic::catch_unwind(|| {
            let d = LoroDoc::new();
            let res = d.import(&input);
            if res.is_ok() { let _ = d.get_deep_value(); let _ = d.export(ExportMode::Snapshot); }
            res.is_ok()
        });
        let el = t.elapsed();
        if el > worst { worst = el; }
        if el > Duration::from_millis(500) { slow += 1; }
        runs += 1;
        match r {
            Ok(true) => ok += 1,
            Ok(false) => err += 1,
            Err(e) => {
                panics += 1;
                if panic_samples.len() < 5 {
                    let msg = e.downcast_ref::<String>().cloned().or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default();
                    panic_samples.push((input.len(), msg.chars().take(160).collect()));
                    std::fs::write(format!("crash_{}.bin", panic_samples.len()), &input).ok();
                }
            }
        }
    }
    println!("runs={runs} accepted={ok} rejected={err} PANICS={panics} slow(>500ms)={slow} worst={worst:?}");
    for (len, msg) in panic_samples { println!("panic on {len}-byte input: {msg}"); }
}
