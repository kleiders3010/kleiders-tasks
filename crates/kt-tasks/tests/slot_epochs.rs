//! Document epochs (D24) through mailbox slots: the proposal travels in the proposer's
//! slot, members switch, and slot sizes drop back to the live state.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use kt_engine::replica::Replica;
use kt_engine::sandbox::SandboxConfig;
use kt_tasks as tk;

fn sandbox() -> SandboxConfig {
    SandboxConfig { worker: PathBuf::from(env!("CARGO_BIN_EXE_kt-tasks-worker")), timeout: Duration::from_secs(20) }
}

fn dev(tag: &str) -> Replica {
    let root = std::env::temp_dir().join(format!("kt-se-{tag}-{}-{}", std::process::id(), rand::random::<u32>()));
    let mut r = Replica::open(&root.join("identity"), &root.join("data"), sandbox()).unwrap();
    r.set_semantics(Box::new(tk::TaskSemantics));
    r
}

#[derive(Default)]
struct Mailbox {
    slots: HashMap<[u8; 32], HashMap<[u8; 32], Vec<u8>>>,
}

impl Mailbox {
    fn sync(&mut self, devs: &mut [&mut Replica]) {
        for _ in 0..3 {
            for r in devs.iter_mut() {
                for t in r.slot_targets() {
                    let blob = r.slot_blob(&t).unwrap();
                    self.slots.entry(t.topic).or_default().insert(t.writer_seed, blob);
                }
            }
            for r in devs.iter_mut() {
                for t in r.read_topics() {
                    if let Some(ws) = self.slots.get(&t.topic) {
                        let blobs: Vec<&[u8]> = ws.values().map(|b| b.as_slice()).collect();
                        r.open_slots(&t, &blobs).unwrap();
                    }
                }
            }
        }
    }
    fn bytes(&self) -> usize {
        self.slots.values().flat_map(|w| w.values()).map(|b| b.len()).sum()
    }
}

fn titles(r: &Replica, g: &kt_engine::GroupId, d: &kt_engine::DocId) -> Vec<String> {
    let mut v: Vec<String> = tk::tasks(r.doc(g, d).unwrap()).into_iter().map(|t| t.title).collect();
    v.sort();
    v
}

#[test]
fn document_epoch_through_slots_shrinks_them() {
    let mut mb = Mailbox::default();
    let (mut a, mut b) = (dev("a"), dev("b"));
    let g = a.create_group().unwrap();
    let d = a.create_doc(&g).unwrap();
    tk::init_list(a.doc(&g, &d).unwrap(), "Groceries").unwrap();
    a.commit(&g, &d).unwrap();
    a.add_member(&g, b.device_public()).unwrap();
    b.accept_group(g);
    for m in a.export_group(&g).unwrap() {
        let _ = b.receive(&m);
    }
    let _ = a.outbox();

    // Lots of churn: history grows, the live list stays small.
    for i in 0..150 {
        let id = tk::add_task(a.doc(&g, &d).unwrap(), None, &format!("item {i}")).unwrap();
        a.commit(&g, &d).unwrap();
        if i % 10 != 0 {
            tk::delete_task(a.doc(&g, &d).unwrap(), &id).unwrap();
            a.commit(&g, &d).unwrap();
        }
    }
    tk::add_task(b.doc(&g, &d).unwrap(), None, "from B").unwrap();
    b.commit(&g, &d).unwrap();
    mb.sync(&mut [&mut a, &mut b]);
    assert_eq!(titles(&a, &g, &d), titles(&b, &g, &d));
    let before = mb.bytes();

    a.propose_doc_epoch(&g, &d).unwrap();
    mb.sync(&mut [&mut a, &mut b]);
    assert_eq!(b.doc_generation(&g, &d), Some(1), "B switched through A's slot");
    assert_eq!(titles(&a, &g, &d), titles(&b, &g, &d));
    let after = mb.bytes();
    assert!(after * 2 < before, "slots shrink after a document epoch: {before} -> {after}");

    // Editing continues in the new generation, both ways.
    tk::add_task(b.doc(&g, &d).unwrap(), None, "after epoch B").unwrap();
    b.commit(&g, &d).unwrap();
    tk::add_task(a.doc(&g, &d).unwrap(), None, "after epoch A").unwrap();
    a.commit(&g, &d).unwrap();
    mb.sync(&mut [&mut a, &mut b]);
    let t = titles(&a, &g, &d);
    assert_eq!(t, titles(&b, &g, &d));
    assert!(t.contains(&"after epoch A".to_string()) && t.contains(&"after epoch B".to_string()));
    println!("slot bytes: {before} before the epoch, {after} after");
}

#[test]
fn edits_made_right_before_a_compaction_still_arrive() {
    // A edits and compacts without syncing in between: its last generation-0 ops never
    // reached B. A's slot keeps carrying them (the "prevgen" envelope), so B can verify the
    // proposal and switch.
    let mut mb = Mailbox::default();
    let (mut a, mut b) = (dev("pa"), dev("pb"));
    let g = a.create_group().unwrap();
    let d = a.create_doc(&g).unwrap();
    tk::init_list(a.doc(&g, &d).unwrap(), "L").unwrap();
    tk::add_task(a.doc(&g, &d).unwrap(), None, "first").unwrap();
    a.commit(&g, &d).unwrap();
    a.add_member(&g, b.device_public()).unwrap();
    b.accept_group(g);
    for m in a.export_group(&g).unwrap() {
        let _ = b.receive(&m);
    }
    let _ = a.outbox();
    mb.sync(&mut [&mut a, &mut b]);

    tk::add_task(a.doc(&g, &d).unwrap(), None, "unsynced").unwrap();
    a.commit(&g, &d).unwrap();
    a.propose_doc_epoch(&g, &d).unwrap();
    // B also edits concurrently in generation 0 and must rebase.
    tk::add_task(b.doc(&g, &d).unwrap(), None, "from B").unwrap();
    b.commit(&g, &d).unwrap();
    mb.sync(&mut [&mut a, &mut b]);
    assert_eq!(b.doc_generation(&g, &d), Some(1));
    let t = titles(&b, &g, &d);
    assert!(t.contains(&"unsynced".to_string()), "{t:?}");
    assert_eq!(t, titles(&a, &g, &d));
    assert!(t.contains(&"from B".to_string()));
}
