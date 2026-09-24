//! Invites on every layer at once (DESIGN §5, D55): never reliant on one channel.
//!
//! **Publish** (members, hourly): each open invite's sealed record for the day goes to
//! - the device itself and every member device (served by lookup key over P2P),
//! - same-network LAN beacons,
//! - the Mainline DHT (a mutable item under a key derived from the lookup),
//! - helpers (a slot under the lookup key),
//! - Nostr (an addressable event under a lookup-derived key, so re-posts replace).
//!
//! **Fetch** (joiner): derive the lookup key for today and yesterday, ask every available
//! layer in parallel, take the newest record that opens. Nothing but the code reveals it.

use std::sync::Arc;
use std::time::Duration;

use base64::Engine as _;
use iroh::EndpointAddr;
use kt_engine::crypto;
use kt_engine::invite::{self, Code, CodeKind, InviteRecord};
use nostr::prelude::*;

use crate::dht::DhtDiscovery;
use crate::helper::HelperMailbox;
use crate::lan::Lan;
use crate::mailbox::{Mailbox, SlotWrite};
use crate::nostr::NostrMailbox;
use crate::p2p::P2p;
use crate::sync::Syncer;

pub const INVITE_KIND: u16 = 35713;

/// The layers available right now (any may be missing).
#[derive(Clone, Default)]
pub struct InviteLayers {
    pub p2p: Option<Arc<P2p>>,
    pub dht: Option<Arc<DhtDiscovery>>,
    pub nostr: Option<Arc<NostrMailbox>>,
    pub helpers: Option<Arc<HelperMailbox>>,
    pub lan: Option<Arc<Lan>>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PublishReport {
    pub records: usize,
    pub dht: usize,
    pub nostr: usize,
    pub helpers: usize,
}

fn dht_seed(lookup: &[u8; 32]) -> [u8; 32] {
    crypto::derive(lookup, "kt/invite-dht/v1", b"").0
}

fn writer_seed(lookup: &[u8; 32]) -> [u8; 32] {
    crypto::derive(lookup, "kt/invite-writer/v1", b"").0
}

fn nostr_keys(lookup: &[u8; 32]) -> Option<Keys> {
    Some(Keys::new(SecretKey::from_slice(&crypto::derive(lookup, "kt/invite-nostr/v1", b"").0).ok()?))
}

fn tag16(lookup: &[u8; 32]) -> [u8; 16] {
    lookup[..16].try_into().expect("16")
}

fn today() -> u32 {
    invite::day_of(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0))
}

/// Publish today's records of every open invite on every available layer.
/// `names(group)` = (group name, our display name).
pub async fn publish(syncer: &Arc<Syncer>, layers: &InviteLayers, names: &(dyn Fn(&kt_engine::GroupId) -> (String, String) + Sync)) -> PublishReport {
    let replica = syncer.replica();
    let day = today();
    // Argon2id runs here (cached per invite and day), off the async threads.
    let pubs = {
        let r = replica.clone();
        let names_owned: std::collections::HashMap<kt_engine::GroupId, (String, String)> = {
            let g = r.lock().expect("replica lock").groups();
            g.into_iter().map(|g| (g, names(&g))).collect()
        };
        tokio::task::spawn_blocking(move || {
            r.lock()
                .expect("replica lock")
                .invite_publications(day, &|g| names_owned.get(g).cloned().unwrap_or_default())
                .unwrap_or_default()
        })
        .await
        .unwrap_or_default()
    };
    let mut report = PublishReport { records: pubs.len(), ..Default::default() };
    if let Some(p) = &layers.p2p {
        p.set_invite_board(pubs.clone());
    }
    if let Some(l) = &layers.lan {
        l.set_extra(pubs.iter().map(|(k, v)| (tag16(k), v.clone())).collect());
    }
    for (lookup, sealed) in &pubs {
        if let Some(d) = &layers.dht {
            if d.put_value(dht_seed(lookup), sealed).await {
                report.dht += 1;
            }
        }
        if let Some(h) = &layers.helpers {
            let w = SlotWrite { topic: *lookup, writer_seed: writer_seed(lookup), blob: sealed.clone() };
            if matches!(h.write(&w).await, crate::mailbox::WriteOutcome::Stored { .. }) {
                report.helpers += 1;
            }
        }
        if let (Some(n), Some(keys)) = (&layers.nostr, nostr_keys(lookup)) {
            let t = hex::encode(tag16(lookup));
            let content = base64::engine::general_purpose::STANDARD.encode(sealed);
            if let Ok(ev) = EventBuilder::new(Kind::Custom(INVITE_KIND), content).tags([Tag::identifier(t.clone()), Tag::hashtag(t)]).finalize(&keys) {
                if n.publish_on(lookup, ev).await > 0 {
                    report.nostr += 1;
                }
            }
        }
    }
    report
}

/// Where a fetched record came from (diagnostics, tests).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Source {
    Direct,
    Lan,
    Dht,
    Helpers,
    Nostr,
}

/// Find the record for a code on every layer at once; newest valid record wins.
/// `direct`: device addresses carried by a link or QR. `lan_wait`: how long to listen.
pub async fn fetch(layers: &InviteLayers, code: &Code, kind: CodeKind, direct: &[EndpointAddr], lan_wait: Duration) -> Option<(InviteRecord, Source)> {
    let day = today();
    for d in [day, day.saturating_sub(1)] {
        let c = *code;
        let Ok(Ok((lookup, key))) = tokio::task::spawn_blocking(move || invite::derive(&c, kind, d)).await else { continue };
        let tag = tag16(&lookup);
        if let Some(l) = &layers.lan {
            l.watch(tag);
        }
        let mut found: Vec<(Vec<u8>, Source)> = Vec::new();
        let direct_f = async {
            let mut v = Vec::new();
            if let Some(p) = &layers.p2p {
                for a in direct {
                    if let Some(b) = p.fetch_invite(a.clone(), lookup).await {
                        v.push((b, Source::Direct));
                    }
                }
            }
            v
        };
        let dht_f = async {
            match &layers.dht {
                Some(dh) => dh.get_value(dht_seed(&lookup)).await.map(|b| vec![(b, Source::Dht)]).unwrap_or_default(),
                None => vec![],
            }
        };
        let helpers_f = async {
            match &layers.helpers {
                Some(h) => h.read(&[lookup]).await.into_iter().map(|s| (s.blob, Source::Helpers)).collect(),
                None => vec![],
            }
        };
        let nostr_f = async {
            match &layers.nostr {
                Some(n) => {
                    let t = hex::encode(tag);
                    let f = Filter::new().kind(Kind::Custom(INVITE_KIND)).hashtag(t).limit(20);
                    n.query_on(&lookup, f)
                        .await
                        .into_iter()
                        .filter_map(|e| base64::engine::general_purpose::STANDARD.decode(&e.content).ok())
                        .map(|b| (b, Source::Nostr))
                        .collect()
                }
                None => vec![],
            }
        };
        let lan_f = async {
            match &layers.lan {
                Some(l) => {
                    tokio::time::sleep(lan_wait).await;
                    l.take_heard(&tag).into_iter().map(|b| (b, Source::Lan)).collect()
                }
                None => vec![],
            }
        };
        let (a, b, c2, e, f) = tokio::join!(direct_f, dht_f, helpers_f, nostr_f, lan_f);
        for v in [a, b, c2, e, f] {
            found.extend(v);
        }
        let best = found
            .into_iter()
            .filter_map(|(b, src)| invite::open_record(&b, &lookup, &key).ok().filter(|r| r.kind == kind).map(|r| (r, src)))
            .max_by_key(|(r, _)| r.published_ms);
        if best.is_some() {
            return best;
        }
    }
    None
}

/// Joiner, after [`fetch`]: begin the join (or link), then hand the request directly to
/// the device in the record (if reachable). Slots and helpers cover the rest.
pub async fn join(syncer: &Arc<Syncer>, p2p: Option<&Arc<P2p>>, rec: &InviteRecord) -> Result<bool, String> {
    {
        let r = syncer.replica();
        let mut r = r.lock().expect("replica lock");
        match rec.kind {
            CodeKind::Invite => r.begin_join(rec),
            CodeKind::Link => r.begin_link(rec),
        }
        .map_err(|e| e.to_string())?;
    }
    if let (Some(p), Some(w)) = (p2p, &rec.whereabouts) {
        if let Some(addr) = postcard::from_bytes::<crate::p2p::WhereData>(w).ok().and_then(|d| d.to_addr()) {
            p.publish_whereabouts();
            return Ok(p.join_via(rec.group, addr).await);
        }
    }
    Ok(false)
}
