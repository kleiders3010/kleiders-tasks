//! Nostr mailbox (DESIGN §7).
//!
//! - One **addressable event kind** for slots ([`SLOT_KIND`], unregistered in the NIPs list
//!   as of 2026-09-24; NIP-78's 30078 and NIP-37's 31234 are taken).
//! - Writer key per device, group and epoch (from the engine's `writer_seed`).
//! - Slot blob split into chunks ≤ [`CHUNK_MAX`] bytes; two alternating buffers so a reader
//!   always finds one complete generation while the other is being rewritten.
//! - Discovery tag `t` = first 16 bytes of the rotating topic (hex).
//! - Quiet-citizen policy (D11): a token bucket per relay shared by all groups, stop at the
//!   first `rate-limited:` with exponential backoff and jitter, each topic uses a subset of
//!   relays chosen from the topic.
//! - Relay health by reading back: "OK" doesn't mean stored.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use base64::Engine as _;
use futures_util::{SinkExt, StreamExt};
use nostr::prelude::*;
use tokio::sync::{mpsc, oneshot};
use tokio_websockets::{ClientBuilder, Message as WsMessage};

use crate::mailbox::{BoxFut, Capabilities, Mailbox, RateBudget, SlotRead, SlotWrite, WriteOutcome};

pub const SLOT_KIND: u16 = 35711;
/// Raw bytes per chunk: base64 + JSON stays under strfry's 64 KiB default event limit (D12).
pub const CHUNK_MAX: usize = 40 * 1024;
pub const MAX_CHUNKS: usize = 16;
const CHUNK_VERSION: u8 = 1;
const HEADER_LEN: usize = 1 + 8 + 1 + 1 + 16;
/// Close an idle relay connection after this long (don't hold sockets open for nothing).
const IDLE_CLOSE: Duration = Duration::from_secs(90);
const OP_TIMEOUT: Duration = Duration::from_secs(20);

/// The relay write policy of D11.
#[derive(Clone, Debug)]
pub struct NostrPolicy {
    pub budget: RateBudget,
    /// How many relays each topic is written to.
    pub relays_per_topic: usize,
    /// How many relays each topic is read from (a little wider than writes, to tolerate
    /// devices that disagree on relay health).
    pub read_relays_per_topic: usize,
    pub backoff_start: Duration,
    pub backoff_max: Duration,
}

impl Default for NostrPolicy {
    fn default() -> Self {
        NostrPolicy {
            budget: RateBudget { per_minute: 6.0, burst: 5 },
            relays_per_topic: 3,
            read_relays_per_topic: 4,
            backoff_start: Duration::from_secs(60),
            backoff_max: Duration::from_secs(3600),
        }
    }
}

// ---------------------------------------------------------------- chunks

struct Chunk {
    generation: u64,
    index: u8,
    count: u8,
    hash: [u8; 16],
    data: Vec<u8>,
}

fn blob_hash(blob: &[u8]) -> [u8; 16] {
    let h = blake3::hash(blob);
    h.as_bytes()[..16].try_into().expect("16")
}

fn encode_chunks(blob: &[u8], generation: u64) -> Option<Vec<String>> {
    let parts: Vec<&[u8]> = if blob.is_empty() { vec![&[][..]] } else { blob.chunks(CHUNK_MAX).collect() };
    if parts.len() > MAX_CHUNKS {
        return None;
    }
    let hash = blob_hash(blob);
    Some(
        parts
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let mut v = Vec::with_capacity(HEADER_LEN + p.len());
                v.push(CHUNK_VERSION);
                v.extend_from_slice(&generation.to_le_bytes());
                v.push(i as u8);
                v.push(parts.len() as u8);
                v.extend_from_slice(&hash);
                v.extend_from_slice(p);
                base64::engine::general_purpose::STANDARD.encode(v)
            })
            .collect(),
    )
}

fn decode_chunk(content: &str) -> Option<Chunk> {
    let v = base64::engine::general_purpose::STANDARD.decode(content).ok()?;
    if v.len() < HEADER_LEN || v[0] != CHUNK_VERSION {
        return None;
    }
    let generation = u64::from_le_bytes(v[1..9].try_into().ok()?);
    let (index, count) = (v[9], v[10]);
    if count == 0 || index >= count || count as usize > MAX_CHUNKS {
        return None;
    }
    Some(Chunk { generation, index, count, hash: v[11..27].try_into().ok()?, data: v[HEADER_LEN..].to_vec() })
}

/// Newest complete generation among one writer's chunk events.
fn assemble(chunks: Vec<Chunk>) -> Option<(u64, Vec<u8>)> {
    let mut by_gen: HashMap<u64, Vec<Chunk>> = HashMap::new();
    for c in chunks {
        by_gen.entry(c.generation).or_default().push(c);
    }
    let mut gens: Vec<u64> = by_gen.keys().copied().collect();
    gens.sort_unstable_by(|a, b| b.cmp(a));
    for g in gens {
        let mut cs = by_gen.remove(&g).expect("key");
        cs.sort_by_key(|c| c.index);
        cs.dedup_by_key(|c| c.index);
        let count = cs[0].count as usize;
        if cs.len() != count || cs.iter().enumerate().any(|(i, c)| c.index as usize != i || c.count as usize != count || c.hash != cs[0].hash) {
            continue;
        }
        let blob: Vec<u8> = cs.iter().flat_map(|c| c.data.iter().copied()).collect();
        if blob_hash(&blob) == cs[0].hash {
            return Some((g, blob));
        }
    }
    None
}

pub fn topic_tag(topic: &[u8; 32]) -> String {
    hex::encode(&topic[..16])
}

fn writer_keys(seed: &[u8; 32]) -> Keys {
    // A seed that isn't a valid secp256k1 scalar (probability ~2^-128) is rehashed.
    let mut s = *seed;
    loop {
        if let Ok(sk) = SecretKey::from_slice(&s) {
            return Keys::new(sk);
        }
        s = *blake3::hash(&s).as_bytes();
    }
}

// ---------------------------------------------------------------- one relay

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RelayHealth {
    pub published_ok: u64,
    pub published_refused: u64,
    pub rate_limited: u64,
    pub connect_failures: u64,
    /// Our own latest slot came back when reading.
    pub readback_hits: u64,
    /// We read and our own latest slot was missing or older ("OK" but not kept).
    pub readback_misses: u64,
}

enum Cmd {
    Publish(Event, oneshot::Sender<(bool, String)>),
    Query(Vec<Filter>, oneshot::Sender<Vec<Event>>),
}

struct Bucket {
    tokens: f64,
    last: Instant,
    blocked_until: Option<Instant>,
    backoff: Duration,
}

pub struct RelayConn {
    pub url: String,
    tx: mpsc::Sender<Cmd>,
    bucket: Mutex<Bucket>,
    health: Mutex<RelayHealth>,
    policy: NostrPolicy,
}

impl RelayConn {
    pub fn new(url: &str, policy: NostrPolicy) -> Arc<RelayConn> {
        let (tx, rx) = mpsc::channel(64);
        let conn = Arc::new(RelayConn {
            url: url.to_string(),
            tx,
            bucket: Mutex::new(Bucket { tokens: policy.budget.burst as f64, last: Instant::now(), blocked_until: None, backoff: policy.backoff_start }),
            health: Mutex::new(RelayHealth::default()),
            policy,
        });
        tokio::spawn(relay_task(url.to_string(), rx, Arc::downgrade(&conn)));
        conn
    }

    pub fn health(&self) -> RelayHealth {
        self.health.lock().expect("lock").clone()
    }

    /// Take `n` write tokens, or say how long to wait.
    fn take(&self, n: u32) -> Result<(), Duration> {
        let mut b = self.bucket.lock().expect("lock");
        let now = Instant::now();
        if let Some(until) = b.blocked_until {
            if now < until {
                return Err(until - now);
            }
            b.blocked_until = None;
        }
        let per_sec = self.policy.budget.per_minute / 60.0;
        b.tokens = (b.tokens + now.duration_since(b.last).as_secs_f64() * per_sec).min(self.policy.budget.burst as f64);
        b.last = now;
        if b.tokens + 1e-9 >= n as f64 {
            b.tokens -= n as f64;
            Ok(())
        } else {
            Err(Duration::from_secs_f64((n as f64 - b.tokens) / per_sec))
        }
    }

    fn rate_limited(&self) {
        let mut b = self.bucket.lock().expect("lock");
        let jitter = rand::random::<f64>() * 0.5 + 0.75;
        let wait = b.backoff.mul_f64(jitter);
        b.blocked_until = Some(Instant::now() + wait);
        b.backoff = (b.backoff * 2).min(self.policy.backoff_max);
        b.tokens = 0.0;
        self.health.lock().expect("lock").rate_limited += 1;
    }

    fn write_succeeded(&self) {
        self.bucket.lock().expect("lock").backoff = self.policy.backoff_start;
    }

    pub async fn publish(&self, ev: Event) -> Option<(bool, String)> {
        let (tx, rx) = oneshot::channel();
        self.tx.send(Cmd::Publish(ev, tx)).await.ok()?;
        tokio::time::timeout(OP_TIMEOUT, rx).await.ok()?.ok()
    }

    pub async fn query(&self, filters: Vec<Filter>) -> Option<Vec<Event>> {
        let (tx, rx) = oneshot::channel();
        self.tx.send(Cmd::Query(filters, tx)).await.ok()?;
        tokio::time::timeout(OP_TIMEOUT, rx).await.ok()?.ok()
    }
}

/// Owns the websocket: connects on demand, closes when idle, fails pending operations on
/// disconnect (callers see a timeout / closed channel and treat the relay as unavailable).
async fn relay_task(url: String, mut rx: mpsc::Receiver<Cmd>, owner: std::sync::Weak<RelayConn>) {
    let mut pending_cmd: Option<Cmd> = None;
    loop {
        let first = match pending_cmd.take() {
            Some(c) => c,
            None => match rx.recv().await {
                Some(c) => c,
                None => return,
            },
        };
        let uri = match http::Uri::try_from(url.as_str()) {
            Ok(u) => u,
            Err(_) => return,
        };
        let connect = tokio::time::timeout(OP_TIMEOUT, ClientBuilder::from_uri(uri).connect()).await;
        let mut ws = match connect {
            Ok(Ok((ws, _))) => ws,
            _ => {
                if let Some(o) = owner.upgrade() {
                    o.health.lock().expect("lock").connect_failures += 1;
                }
                drop(first); // fails the waiting caller
                continue;
            }
        };
        let mut ok_waiters: HashMap<EventId, oneshot::Sender<(bool, String)>> = HashMap::new();
        let mut queries: HashMap<String, (Vec<Event>, oneshot::Sender<Vec<Event>>)> = HashMap::new();
        let mut next = Some(first);
        let mut sub_counter = 0u64;
        loop {
            if let Some(cmd) = next.take() {
                let text = match cmd {
                    Cmd::Publish(ev, tx) => {
                        ok_waiters.insert(ev.id, tx);
                        ClientMessage::event(ev).as_json()
                    }
                    Cmd::Query(filters, tx) => {
                        sub_counter += 1;
                        let sid = format!("q{sub_counter}{:08x}", rand::random::<u32>());
                        queries.insert(sid.clone(), (Vec::new(), tx));
                        ClientMessage::req(SubscriptionId::new(&sid), filters).as_json()
                    }
                };
                if ws.send(WsMessage::text(text)).await.is_err() {
                    break;
                }
            }
            let idle = ok_waiters.is_empty() && queries.is_empty();
            tokio::select! {
                cmd = rx.recv() => match cmd {
                    Some(c) => next = Some(c),
                    None => { let _ = ws.close().await; return; }
                },
                msg = ws.next() => {
                    let Some(Ok(msg)) = msg else { break };
                    let Some(text) = msg.as_text() else { continue };
                    let Ok(rm) = RelayMessage::from_json(text) else { continue };
                    match rm {
                        RelayMessage::Ok { event_id, status, message } => {
                            if let Some(tx) = ok_waiters.remove(&event_id) {
                                let _ = tx.send((status, message.to_string()));
                            }
                        }
                        RelayMessage::Event { subscription_id, event } => {
                            if let Some((evs, _)) = queries.get_mut(subscription_id.as_str()) {
                                if evs.len() < 5000 {
                                    evs.push(event.into_owned());
                                }
                            }
                        }
                        RelayMessage::EndOfStoredEvents(sid) => {
                            if let Some((evs, tx)) = queries.remove(sid.as_str()) {
                                let _ = tx.send(evs);
                                let _ = ws.send(WsMessage::text(ClientMessage::close(sid.into_owned()).as_json())).await;
                            }
                        }
                        RelayMessage::Closed { subscription_id, .. } => {
                            if let Some((evs, tx)) = queries.remove(subscription_id.as_str()) {
                                let _ = tx.send(evs);
                            }
                        }
                        _ => {}
                    }
                },
                _ = tokio::time::sleep(IDLE_CLOSE), if idle => {
                    let _ = ws.close().await;
                    break;
                }
            }
        }
        // Disconnected: dropping the waiters fails their callers.
        drop(ok_waiters);
        drop(queries);
        pending_cmd = next.take();
    }
}

// ---------------------------------------------------------------- the mailbox

pub struct NostrMailbox {
    relays: Vec<Arc<RelayConn>>,
    policy: NostrPolicy,
    /// Last generation written per writer pubkey (strictly increasing created_at / generation).
    last_gen: Mutex<HashMap<[u8; 32], u64>>,
    /// What we last stored per (relay, writer), for read-back health.
    stored: Mutex<HashMap<(usize, [u8; 32]), u64>>,
}

impl NostrMailbox {
    pub fn new(relay_urls: &[&str], policy: NostrPolicy) -> NostrMailbox {
        NostrMailbox {
            relays: relay_urls.iter().map(|u| RelayConn::new(u, policy.clone())).collect(),
            policy,
            last_gen: Mutex::new(HashMap::new()),
            stored: Mutex::new(HashMap::new()),
        }
    }

    pub fn health(&self) -> Vec<(String, RelayHealth)> {
        self.relays.iter().map(|r| (r.url.clone(), r.health())).collect()
    }

    /// Relays for a topic, best first: a stable per-topic ranking (so writers and readers
    /// agree without talking), with relays that failed read-back moved to the end.
    fn ranked(&self, topic: &[u8; 32]) -> Vec<usize> {
        let mut idx: Vec<(bool, [u8; 32], usize)> = self
            .relays
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let h = r.health();
                let bad = h.readback_misses > h.readback_hits + 2;
                let mut m = topic.to_vec();
                m.extend_from_slice(r.url.as_bytes());
                (bad, *blake3::hash(&m).as_bytes(), i)
            })
            .collect();
        idx.sort();
        idx.into_iter().map(|x| x.2).collect()
    }

    fn next_generation(&self, writer: [u8; 32]) -> u64 {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let mut m = self.last_gen.lock().expect("lock");
        let g = m.get(&writer).map(|l| now.max(l + 1)).unwrap_or(now);
        m.insert(writer, g);
        g
    }

    async fn write_impl(&self, w: &SlotWrite) -> WriteOutcome {
        let keys = writer_keys(&w.writer_seed);
        let writer = keys.public_key().to_bytes();
        let generation = self.next_generation(writer);
        let Some(chunks) = encode_chunks(&w.blob, generation) else {
            return WriteOutcome::Refused(format!("slot of {} bytes exceeds {} chunks", w.blob.len(), MAX_CHUNKS));
        };
        let buffer = generation % 2;
        let tag = topic_tag(&w.topic);
        let mut events = Vec::new();
        for (i, content) in chunks.into_iter().enumerate() {
            let ev = EventBuilder::new(Kind::Custom(SLOT_KIND), content)
                .tags([Tag::identifier(format!("{buffer}.{i}")), Tag::hashtag(&tag)])
                .custom_created_at(Timestamp::from_secs(generation))
                .finalize(&keys);
            match ev {
                Ok(e) => events.push(e),
                Err(e) => return WriteOutcome::Refused(format!("signing failed: {e}")),
            }
        }
        let targets: Vec<usize> = self.ranked(&w.topic).into_iter().take(self.policy.relays_per_topic).collect();
        let mut futs = Vec::new();
        for ri in targets.iter().copied() {
            let relay = self.relays[ri].clone();
            let evs = events.clone();
            futs.push(async move {
                if let Err(wait) = relay.take(evs.len() as u32) {
                    return (ri, RelayResult::Deferred(wait));
                }
                for ev in evs {
                    match relay.publish(ev).await {
                        Some((true, _)) => {}
                        Some((false, msg)) if msg.starts_with("rate-limited") => {
                            relay.rate_limited();
                            return (ri, RelayResult::Deferred(Duration::from_secs(60)));
                        }
                        Some((false, msg)) => {
                            relay.health.lock().expect("lock").published_refused += 1;
                            return (ri, RelayResult::Refused(msg));
                        }
                        None => return (ri, RelayResult::Unavailable),
                    }
                }
                relay.write_succeeded();
                relay.health.lock().expect("lock").published_ok += 1;
                (ri, RelayResult::Stored)
            });
        }
        let results = futures_util::future::join_all(futs).await;
        let mut stored = 0;
        let mut wait: Option<Duration> = None;
        let mut refused = None;
        for (ri, r) in &results {
            match r {
                RelayResult::Stored => {
                    stored += 1;
                    self.stored.lock().expect("lock").insert((*ri, writer), generation);
                }
                RelayResult::Deferred(d) => wait = Some(wait.map_or(*d, |w: Duration| w.min(*d))),
                RelayResult::Refused(m) => refused = Some(m.clone()),
                RelayResult::Unavailable => {}
            }
        }
        if stored > 0 {
            WriteOutcome::Stored { stored, tried: results.len() }
        } else if let Some(d) = wait {
            WriteOutcome::Deferred { retry_after: d }
        } else if let Some(m) = refused {
            WriteOutcome::Refused(m)
        } else {
            WriteOutcome::Unavailable
        }
    }

    async fn read_impl(&self, topics: &[[u8; 32]]) -> Vec<SlotRead> {
        // Which relays to ask, and for which topics.
        let mut per_relay: HashMap<usize, Vec<[u8; 32]>> = HashMap::new();
        for t in topics {
            for ri in self.ranked(t).into_iter().take(self.policy.read_relays_per_topic) {
                per_relay.entry(ri).or_default().push(*t);
            }
        }
        let futs = per_relay.into_iter().map(|(ri, ts)| {
            let relay = self.relays[ri].clone();
            async move {
                let tags: Vec<String> = ts.iter().map(topic_tag).collect();
                let filter = Filter::new().kind(Kind::Custom(SLOT_KIND)).custom_tags(SingleLetterTag::from_char('t').expect("t"), tags);
                (ri, ts, relay.query(vec![filter]).await)
            }
        });
        let results = futures_util::future::join_all(futs).await;
        // (topic, writer) -> chunks from all relays
        let mut chunks: HashMap<([u8; 32], [u8; 32]), Vec<Chunk>> = HashMap::new();
        for (ri, ts, evs) in results {
            let Some(evs) = evs else { continue };
            let tag_to_topic: HashMap<String, [u8; 32]> = ts.iter().map(|t| (topic_tag(t), *t)).collect();
            let mut newest_here: HashMap<[u8; 32], u64> = HashMap::new();
            for ev in evs {
                if ev.kind != Kind::Custom(SLOT_KIND) || ev.verify().is_err() {
                    continue;
                }
                let Some(topic) = ev.tags.iter().find(|t| t.kind() == "t").and_then(|t| t.content()).and_then(|c| tag_to_topic.get(c)) else {
                    continue;
                };
                let Some(c) = decode_chunk(&ev.content) else { continue };
                let writer = ev.pubkey.to_bytes();
                let e = newest_here.entry(writer).or_insert(0);
                *e = (*e).max(c.generation);
                chunks.entry((*topic, writer)).or_default().push(c);
            }
            // Read-back health: did this relay return what we stored on it?
            let stored = self.stored.lock().expect("lock").clone();
            let relay = &self.relays[ri];
            for ((sri, writer), gen_) in stored {
                if sri != ri {
                    continue;
                }
                let mut h = relay.health.lock().expect("lock");
                match newest_here.get(&writer) {
                    Some(g) if *g >= gen_ => h.readback_hits += 1,
                    _ => h.readback_misses += 1,
                }
            }
        }
        let mut out = Vec::new();
        for ((topic, writer), cs) in chunks {
            if let Some((version, blob)) = assemble(cs) {
                out.push(SlotRead { topic, writer, version, blob });
            }
        }
        out
    }
}

enum RelayResult {
    Stored,
    Deferred(Duration),
    Refused(String),
    Unavailable,
}

impl Mailbox for NostrMailbox {
    fn name(&self) -> String {
        format!("nostr({} relays)", self.relays.len())
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities {
            replace: true,
            max_slot: CHUNK_MAX * MAX_CHUNKS,
            ttl: None,
            read_auth: false,
            subscribe: false,
            write_budget: Some(self.policy.budget),
        }
    }

    fn writer_id(&self, writer_seed: &[u8; 32]) -> [u8; 32] {
        writer_keys(writer_seed).public_key().to_bytes()
    }

    fn write<'a>(&'a self, w: &'a SlotWrite) -> BoxFut<'a, WriteOutcome> {
        Box::pin(self.write_impl(w))
    }

    fn read<'a>(&'a self, topics: &'a [[u8; 32]]) -> BoxFut<'a, Vec<SlotRead>> {
        Box::pin(self.read_impl(topics))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunks_roundtrip_and_pick_newest_complete() {
        let blob: Vec<u8> = (0..100_000u32).map(|i| (i * 7) as u8).collect();
        let enc = encode_chunks(&blob, 10).unwrap();
        assert_eq!(enc.len(), 3);
        for e in &enc {
            assert!(e.len() < 60 * 1024);
        }
        let newer = encode_chunks(b"newer", 11).unwrap();
        // Newer generation incomplete? Fall back to the complete older one.
        let mut cs: Vec<Chunk> = enc.iter().map(|c| decode_chunk(c).unwrap()).collect();
        let (g, b) = assemble(cs.drain(..).collect()).unwrap();
        assert_eq!((g, b), (10, blob.clone()));
        let mut cs: Vec<Chunk> = enc.iter().map(|c| decode_chunk(c).unwrap()).collect();
        cs.extend(newer.iter().map(|c| decode_chunk(c).unwrap()));
        assert_eq!(assemble(cs).unwrap(), (11, b"newer".to_vec()));
        // Missing a chunk: not complete.
        let cs: Vec<Chunk> = enc.iter().skip(1).map(|c| decode_chunk(c).unwrap()).collect();
        assert!(assemble(cs).is_none());
        assert!(decode_chunk("not base64!").is_none());
        assert!(encode_chunks(&vec![0u8; CHUNK_MAX * MAX_CHUNKS + 1], 1).is_none());
    }
}
