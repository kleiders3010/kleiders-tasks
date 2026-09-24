//! Sync through mailbox slots only (DESIGN §7): replaceable per-device slots under rotating
//! topics, forward slots across key epochs, removal through slots.

use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use kt_engine::replica::Replica;
use kt_engine::sandbox::SandboxConfig;
use kt_engine::{DocId, GroupId};

fn sandbox() -> SandboxConfig {
    SandboxConfig { worker: PathBuf::from(env!("CARGO_BIN_EXE_kt-import-worker")), timeout: Duration::from_secs(20) }
}

fn dev(tag: &str) -> Replica {
    let root = std::env::temp_dir().join(format!("kt-slot-{tag}-{}-{}", std::process::id(), rand::random::<u32>()));
    Replica::open(&root.join("identity"), &root.join("data"), sandbox()).unwrap()
}

/// A replace-semantics mailbox: topic -> writer -> latest blob (like Nostr addressable events).
#[derive(Default)]
struct Mailbox {
    slots: HashMap<[u8; 32], HashMap<[u8; 32], Vec<u8>>>,
}

impl Mailbox {
    fn write(&mut self, r: &mut Replica) {
        for t in r.slot_targets() {
            let blob = r.slot_blob(&t).unwrap();
            self.slots.entry(t.topic).or_default().insert(t.writer_seed, blob);
        }
    }
    fn read(&self, r: &mut Replica) {
        // Two passes: the first can reveal new epochs (and so new topics) to read.
        for _ in 0..2 {
            for t in r.read_topics() {
                if let Some(ws) = self.slots.get(&t.topic) {
                    let blobs: Vec<&[u8]> = ws.values().map(|b| b.as_slice()).collect();
                    r.open_slots(&t, &blobs).unwrap();
                }
            }
        }
    }
    fn sync(&mut self, devs: &mut [&mut Replica]) {
        for _ in 0..2 {
            for d in devs.iter_mut() {
                self.write(d);
            }
            for d in devs.iter_mut() {
                self.read(d);
            }
        }
    }
    fn blobs(&self) -> usize {
        self.slots.values().map(|w| w.len()).sum()
    }
}

fn set(r: &mut Replica, g: &GroupId, d: &DocId, k: &str, v: &str) {
    r.doc(g, d).unwrap().get_map("tasks").insert(k, v).unwrap();
    r.commit(g, d).unwrap();
}

fn value(r: &Replica, g: &GroupId, d: &DocId) -> String {
    format!("{:?}", r.doc(g, d).map(|x| x.get_deep_value()))
}

/// Join directly (M3 replaces this with invites); everything after goes through slots.
fn join(owner: &mut Replica, joiner: &mut Replica, g: GroupId) {
    owner.add_member(&g, joiner.device_public()).unwrap();
    joiner.accept_group(g);
    for m in owner.export_group(&g).unwrap() {
        let _ = joiner.receive(&m);
    }
}

#[test]
fn devices_sync_through_slots_only() {
    let mut mb = Mailbox::default();
    let (mut a, mut b) = (dev("a"), dev("b"));
    let g = a.create_group().unwrap();
    let d = a.create_doc(&g).unwrap();
    set(&mut a, &g, &d, "t1", "milk");
    join(&mut a, &mut b, g);
    let _ = a.outbox();

    set(&mut a, &g, &d, "t2", "from A");
    set(&mut b, &g, &d, "t3", "from B");
    mb.sync(&mut [&mut a, &mut b]);
    let v = value(&a, &g, &d);
    assert_eq!(v, value(&b, &g, &d));
    assert!(v.contains("milk") && v.contains("from A") && v.contains("from B"), "{v}");

    // Replacement: many more edits, still one slot per device and topic.
    let before = mb.blobs();
    for i in 0..20 {
        set(&mut a, &g, &d, &format!("a{i}"), "x");
        mb.sync(&mut [&mut a, &mut b]);
    }
    assert_eq!(mb.blobs(), before);
    assert_eq!(value(&a, &g, &d), value(&b, &g, &d));

    // A topic reveals nothing about the group: it isn't derived from its ID.
    for t in a.read_topics() {
        assert_ne!(t.topic, g.0);
    }
}

#[test]
fn epoch_changes_propagate_through_forward_slots_and_removal_holds() {
    let mut mb = Mailbox::default();
    let (mut a, mut b, mut c) = (dev("fa"), dev("fb"), dev("fc"));
    let g = a.create_group().unwrap();
    let d = a.create_doc(&g).unwrap();
    set(&mut a, &g, &d, "t1", "hello");
    join(&mut a, &mut b, g);
    // C joins; B learns of C's Add epoch only through A's forward slot.
    join(&mut a, &mut c, g);
    let _ = a.outbox();
    mb.sync(&mut [&mut a, &mut b, &mut c]);
    let head = a.epochs(&g).unwrap().head();
    assert_eq!(b.epochs(&g).unwrap().head(), head, "B followed the epoch change via slots");
    set(&mut c, &g, &d, "t2", "from C");
    mb.sync(&mut [&mut a, &mut b, &mut c]);
    assert!(value(&b, &g, &d).contains("from C"));

    // A removes C; C keeps editing without knowing.
    let c_id = c.device_id();
    a.remove_member(&g, c_id).unwrap();
    set(&mut c, &g, &d, "evil", "late edit");
    mb.sync(&mut [&mut a, &mut b, &mut c]);
    set(&mut a, &g, &d, "t3", "after removal");
    mb.sync(&mut [&mut a, &mut b, &mut c]);
    assert!(value(&b, &g, &d).contains("after removal"));
    assert!(!value(&a, &g, &d).contains("late edit"));
    assert!(!value(&b, &g, &d).contains("late edit"));
    assert!(!value(&c, &g, &d).contains("after removal"), "C can't read after removal");
    // C can't even find the new topic.
    let new_topics: Vec<_> = a.read_topics().into_iter().map(|t| t.topic).collect();
    assert!(c.slot_targets().iter().all(|t| !new_topics[..1].contains(&t.topic)) || c.slot_targets().is_empty());
    assert!(c.slot_targets().is_empty(), "a removed device writes no slots");
}

#[test]
fn group_endpoint_sets_spread_through_slots_and_newest_wins() {
    let mut mb = Mailbox::default();
    let (mut a, mut b) = (dev("ma"), dev("mb"));
    let g = a.create_group().unwrap();
    let _d = a.create_doc(&g).unwrap();
    join(&mut a, &mut b, g);
    let _ = a.outbox();
    a.propose_mailbox_set(&g, "nostr", vec!["wss://one".into(), "wss://two".into()]).unwrap();
    a.propose_mailbox_set(&g, "helpers", vec!["h1".into()]).unwrap();
    mb.sync(&mut [&mut a, &mut b]);
    assert_eq!(b.mailbox_set(&g, "nostr").0.unwrap().endpoints, vec!["wss://one", "wss://two"]);
    assert_eq!(b.mailbox_set(&g, "helpers").0.unwrap().endpoints, vec!["h1"]);
    std::thread::sleep(std::time::Duration::from_millis(5));
    b.propose_mailbox_set(&g, "nostr", vec!["wss://two".into(), "wss://three".into()]).unwrap();
    mb.sync(&mut [&mut a, &mut b]);
    let (cur, prev) = a.mailbox_set(&g, "nostr");
    assert_eq!(cur.unwrap().endpoints, vec!["wss://two", "wss://three"]);
    assert_eq!(prev.unwrap().endpoints, vec!["wss://one", "wss://two"], "previous set still read for a while");
}
