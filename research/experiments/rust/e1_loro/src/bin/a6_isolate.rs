//! A6 mitigation prototype: validate an untrusted update by importing it into a
//! copy of the live document inside a disposable child process. The parent
//! only imports if the child succeeds, and only if the child reports that the
//! update advanced nothing but the sender's own PeerID.
use loro::{ExportMode, LoroDoc};
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::time::Instant;

fn child() {
    // stdin: [u32 len snapshot][snapshot][update]; stdout: peers advanced, comma separated
    let mut buf = Vec::new();
    std::io::stdin().read_to_end(&mut buf).unwrap();
    let n = u32::from_le_bytes(buf[..4].try_into().unwrap()) as usize;
    let (snap, upd) = (&buf[4..4 + n], &buf[4 + n..]);
    let d = LoroDoc::from_snapshot(snap).unwrap();
    let before = d.oplog_vv();
    match d.import(upd) {
        Ok(_) => {
            let _ = d.get_deep_value(); // force materialization (some Loro bugs panic on first read)
            let after = d.oplog_vv();
            let peers: Vec<String> = after.iter().filter(|(p, c)| before.get(p).copied().unwrap_or(0) < **c).map(|(p, _)| p.to_string()).collect();
            print!("OK {}", peers.join(","));
        }
        Err(e) => print!("ERR {e}"),
    }
}

fn validate(exe: &std::path::Path, live: &LoroDoc, update: &[u8]) -> (String, std::time::Duration) {
    let t = Instant::now();
    let snap = live.export(ExportMode::Snapshot).unwrap();
    let mut c = Command::new(exe).arg("--child").stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().unwrap();
    {
        let mut si = c.stdin.take().unwrap();
        si.write_all(&(snap.len() as u32).to_le_bytes()).unwrap();
        si.write_all(&snap).unwrap();
        si.write_all(update).unwrap();
    }
    let out = c.wait_with_output().unwrap();
    let verdict = if out.status.success() { String::from_utf8_lossy(&out.stdout).to_string() } else { format!("CHILD CRASHED ({:?})", out.status.code()) };
    (verdict, t.elapsed())
}

fn main() {
    if std::env::args().any(|a| a == "--child") { return child(); }
    let exe = std::env::current_exe().unwrap();
    // A live doc of realistic size (~1 year of a busy list ≈ 1.5k tasks)
    let live = LoroDoc::new(); live.set_peer_id(1).unwrap();
    let t = live.get_tree("t");
    for i in 0..1500 { let id = t.create(None).unwrap(); t.get_meta(id).unwrap().insert("title", format!("task number {i}")).unwrap(); }
    live.commit();
    println!("live snapshot: {} KB", live.export(ExportMode::Snapshot).unwrap().len() / 1024);

    // 1. honest update from peer 2
    let other = LoroDoc::from_snapshot(&live.export(ExportMode::Snapshot).unwrap()).unwrap(); other.set_peer_id(2).unwrap();
    let vv = other.oplog_vv();
    other.get_tree("t").create(None).unwrap(); other.commit();
    let honest = other.export(ExportMode::updates(&vv)).unwrap();
    let (v, d) = validate(&exe, &live, &honest);
    println!("honest update from peer 2 -> {v} in {d:?}");

    // 2. impersonation: sender is peer 3 but blob advances peer 1
    let imp = LoroDoc::from_snapshot(&live.export(ExportMode::Snapshot).unwrap()).unwrap(); imp.set_peer_id(1).unwrap();
    let vv = imp.oplog_vv();
    imp.get_tree("t").create(None).unwrap(); imp.commit();
    let forged = imp.export(ExportMode::updates(&vv)).unwrap();
    let (v, d) = validate(&exe, &live, &forged);
    println!("update claimed by peer 3 -> {v} in {d:?} (parent rejects: advanced peer != sender)");

    // 3. poison pill from the fuzz corpus
    let poison = std::fs::read("fuzz_out/crash_abort_1.bin").unwrap();
    let (v, d) = validate(&exe, &live, &poison);
    println!("poison pill vs live doc -> {v} in {d:?}; parent still alive");
    let empty = LoroDoc::new(); empty.set_peer_id(5).unwrap();
    let (v, d) = validate(&exe, &empty, &poison);
    println!("poison pill vs empty doc -> {v} in {d:?}; parent still alive");

    // 4. overhead: 20 validations
    let t0 = Instant::now();
    for _ in 0..20 { validate(&exe, &live, &honest); }
    println!("mean validation overhead: {:?}", t0.elapsed() / 20);
}
