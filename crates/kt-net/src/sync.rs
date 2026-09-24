//! Drives a [`Replica`] against mailboxes: write own slots, read members' slots.
//!
//! - A slot is rewritten only when its content changed, when it's due for a refresh, or
//!   when reading showed the mailbox lost it (read-back).
//! - A slot version already applied isn't opened again.
//! - Replica work (crypto, sandboxed imports) runs on blocking threads.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use kt_engine::replica::{ReadTopic, Replica};

use crate::mailbox::{Mailbox, SlotWrite, WriteOutcome};

/// Rewrite an unchanged slot after this long (mailboxes may prune old data).
pub const REFRESH: Duration = Duration::from_secs(3 * 24 * 3600);

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WriteReport {
    pub written: usize,
    pub unchanged: usize,
    pub deferred: usize,
    pub failed: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReadReport {
    pub slots_opened: usize,
    pub already_seen: usize,
    pub messages: usize,
    pub rejected: usize,
}

struct Written {
    fingerprint: [u8; 32],
    at: Instant,
}

pub struct Syncer {
    replica: Arc<Mutex<Replica>>,
    mailboxes: Vec<Arc<dyn Mailbox>>,
    /// (mailbox, topic, writer seed) -> what was last written.
    written: Mutex<HashMap<(usize, [u8; 32], [u8; 32]), Written>>,
    /// (mailbox, topic, writer) -> newest version applied.
    seen: Mutex<HashMap<(usize, [u8; 32], [u8; 32]), u64>>,
    /// Slots the last read showed missing (force a rewrite).
    lost: Mutex<HashSet<(usize, [u8; 32], [u8; 32])>>,
}

impl Syncer {
    pub fn new(replica: Arc<Mutex<Replica>>, mailboxes: Vec<Arc<dyn Mailbox>>) -> Arc<Syncer> {
        Arc::new(Syncer {
            replica,
            mailboxes,
            written: Mutex::new(HashMap::new()),
            seen: Mutex::new(HashMap::new()),
            lost: Mutex::new(HashSet::new()),
        })
    }

    pub fn replica(&self) -> Arc<Mutex<Replica>> {
        self.replica.clone()
    }

    /// Write every slot that changed (or is due) to every mailbox.
    pub async fn write_round(&self) -> WriteReport {
        let replica = self.replica.clone();
        let plans = tokio::task::spawn_blocking(move || {
            let mut r = replica.lock().expect("replica lock");
            r.slot_targets()
                .into_iter()
                .filter_map(|t| r.slot_fingerprint(&t).ok().map(|fp| (t, fp)))
                .collect::<Vec<_>>()
        })
        .await
        .unwrap_or_default();
        let mut report = WriteReport::default();
        for (t, fp) in plans {
            let due: Vec<usize> = (0..self.mailboxes.len())
                .filter(|mi| {
                    let key = (*mi, t.topic, t.writer_seed);
                    if self.lost.lock().expect("lock").contains(&key) {
                        return true;
                    }
                    match self.written.lock().expect("lock").get(&key) {
                        Some(w) => w.fingerprint != fp || w.at.elapsed() > REFRESH,
                        None => true,
                    }
                })
                .collect();
            if due.is_empty() {
                report.unchanged += 1;
                continue;
            }
            let replica = self.replica.clone();
            let t2 = t.clone();
            let Ok(Ok(blob)) = tokio::task::spawn_blocking(move || replica.lock().expect("replica lock").slot_blob(&t2)).await else {
                report.failed += 1;
                continue;
            };
            let w = SlotWrite { topic: t.topic, writer_seed: t.writer_seed, blob };
            let outcomes = futures_util::future::join_all(due.iter().map(|mi| self.mailboxes[*mi].write(&w))).await;
            for (mi, o) in due.iter().zip(outcomes) {
                let key = (*mi, t.topic, t.writer_seed);
                match o {
                    WriteOutcome::Stored { .. } => {
                        report.written += 1;
                        self.written.lock().expect("lock").insert(key, Written { fingerprint: fp, at: Instant::now() });
                        self.lost.lock().expect("lock").remove(&key);
                    }
                    WriteOutcome::Deferred { .. } => report.deferred += 1,
                    WriteOutcome::Refused(_) | WriteOutcome::Unavailable => report.failed += 1,
                }
            }
        }
        report
    }

    /// Read members' slots from every mailbox and apply new versions. Repeats while reading
    /// reveals new topics (an epoch change learned from a forward slot).
    pub async fn read_round(&self) -> ReadReport {
        let mut report = ReadReport::default();
        let mut done_topics: HashSet<[u8; 32]> = HashSet::new();
        for _ in 0..3 {
            let replica = self.replica.clone();
            let (topics, my_targets) = tokio::task::spawn_blocking(move || {
                let r = replica.lock().expect("replica lock");
                (r.read_topics(), r.slot_targets())
            })
            .await
            .unwrap_or_default();
            let new_topics: Vec<ReadTopic> = topics.into_iter().filter(|t| !done_topics.contains(&t.topic)).collect();
            if new_topics.is_empty() {
                break;
            }
            let topic_ids: Vec<[u8; 32]> = new_topics.iter().map(|t| t.topic).collect();
            done_topics.extend(topic_ids.iter().copied());
            let reads = futures_util::future::join_all(self.mailboxes.iter().map(|m| m.read(&topic_ids))).await;
            // Group new slot versions by topic.
            let mut by_topic: HashMap<[u8; 32], Vec<Vec<u8>>> = HashMap::new();
            for (mi, slots) in reads.into_iter().enumerate() {
                let own: HashMap<[u8; 32], [u8; 32]> =
                    my_targets.iter().map(|t| (self.mailboxes[mi].writer_id(&t.writer_seed), t.topic)).collect();
                let mut own_seen: HashSet<([u8; 32], [u8; 32])> = HashSet::new();
                for s in slots {
                    if own.get(&s.writer) == Some(&s.topic) {
                        own_seen.insert((s.topic, s.writer));
                        continue;
                    }
                    let key = (mi, s.topic, s.writer);
                    let mut seen = self.seen.lock().expect("lock");
                    if seen.get(&key).is_some_and(|v| *v >= s.version) {
                        report.already_seen += 1;
                        continue;
                    }
                    seen.insert(key, s.version);
                    by_topic.entry(s.topic).or_default().push(s.blob);
                }
                // Read-back: our own slots this mailbox should have returned.
                for t in &my_targets {
                    if !topic_ids.contains(&t.topic) {
                        continue;
                    }
                    let w = self.mailboxes[mi].writer_id(&t.writer_seed);
                    let key = (mi, t.topic, t.writer_seed);
                    if self.written.lock().expect("lock").contains_key(&key) && !own_seen.contains(&(t.topic, w)) {
                        self.lost.lock().expect("lock").insert(key);
                    }
                }
            }
            for t in new_topics {
                let Some(blobs) = by_topic.remove(&t.topic) else { continue };
                let replica = self.replica.clone();
                let res = tokio::task::spawn_blocking(move || {
                    let refs: Vec<&[u8]> = blobs.iter().map(|b| b.as_slice()).collect();
                    (refs.len(), replica.lock().expect("replica lock").open_slots(&t, &refs))
                })
                .await;
                if let Ok((n, Ok(r))) = res {
                    report.slots_opened += n;
                    report.messages += r.messages;
                    report.rejected += r.failed + r.bad_slots;
                }
            }
        }
        report
    }

    /// One full round: send, receive, then send again if receiving changed our slots
    /// (e.g. merge duty, a new generation to rebase onto).
    pub async fn sync_once(&self) -> (WriteReport, ReadReport) {
        let mut w = self.write_round().await;
        let r = self.read_round().await;
        let w2 = self.write_round().await;
        w.written += w2.written;
        w.deferred += w2.deferred;
        w.failed += w2.failed;
        (w, r)
    }

    /// Run until `stop` fires: write rounds every 30–60 s with jitter (D11), reads every
    /// `read_every`.
    pub async fn run(self: Arc<Self>, read_every: Duration, mut stop: tokio::sync::watch::Receiver<bool>) {
        let mut next_write = Instant::now();
        let mut next_read = Instant::now();
        loop {
            if *stop.borrow() {
                return;
            }
            let now = Instant::now();
            if now >= next_write {
                self.write_round().await;
                next_write = Instant::now() + Duration::from_millis(30_000 + rand::random::<u64>() % 30_000);
            }
            if now >= next_read {
                self.read_round().await;
                next_read = Instant::now() + read_every;
            }
            let wake = next_write.min(next_read);
            tokio::select! {
                _ = tokio::time::sleep_until(wake.into()) => {}
                _ = stop.changed() => {}
            }
        }
    }
}
