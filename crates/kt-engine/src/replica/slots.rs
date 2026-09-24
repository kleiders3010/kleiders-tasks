//! Mailbox slots (DESIGN §7, §7a).
//!
//! Per group, each member device keeps one replaceable **slot** per epoch topic. Rewriting
//! a slot replaces the previous version, so a mailbox holds at most one slot per device and
//! group, whatever the number of edits.
//!
//! - **Main slot** (under the current head's topic): this device's own ops since the start
//!   of each document's current generation; the generation's proposal if this device made
//!   it (or its proposer left); and the key-epoch records this device must carry.
//! - **Forward slot** (under a parent epoch's topic, for [`FORWARD_WINDOW_MS`] after an
//!   epoch change): only the epoch records, so members that don't know the new epoch yet
//!   find it under the topic they still read.
//!
//! The whole slot is sealed with its epoch's key (header bound as associated data), so
//! mailboxes see only rotating topics and ciphertext. This also encrypts control messages
//! on every network path. Everything inside stays individually signed and goes through
//! [`Replica::receive`], so a slot is exactly as trusted as its contents.

use serde::{Deserialize, Serialize};

use super::{Message, Replica};
use crate::crypto::{self, SymKey};
use crate::envelope::{Envelope, Inner, MessageKind};
use crate::ids::{EpochId, GroupId};
use crate::{wire, Error, Result};
use loro::{ExportMode, IdSpan};

const SLOT_TAG: [u8; 2] = *b"SL";
const SLOT_VERSION: u16 = 1;
/// How long a device keeps writing forward slots under the previous epoch's topic.
pub const FORWARD_WINDOW_MS: u64 = 30 * 24 * 3600 * 1000;
/// How many recent epochs' topics a device reads (plus all current heads).
pub const READ_EPOCHS: usize = 4;

/// Network-visible slot address, derived from the epoch key: unknown to non-members and
/// changes with every key epoch.
pub fn topic_for(key: &SymKey, epoch: &EpochId) -> [u8; 32] {
    let mut m = Vec::with_capacity(64);
    m.extend_from_slice(&key.0);
    m.extend_from_slice(&epoch.0);
    crypto::hash("kt/topic/v1", &m)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotRole {
    Main,
    Forward,
}

/// A slot this device should write.
#[derive(Clone, Debug)]
pub struct SlotTarget {
    pub group: GroupId,
    pub epoch: EpochId,
    pub topic: [u8; 32],
    pub role: SlotRole,
    /// Seed for the mailbox-level writer key (e.g. a Nostr key): per device, group and
    /// epoch, so a device's slots in different groups or epochs aren't linkable by key.
    pub writer_seed: [u8; 32],
}

/// A topic this device should read.
#[derive(Clone, Debug)]
pub struct ReadTopic {
    pub group: GroupId,
    pub epoch: EpochId,
    pub topic: [u8; 32],
}

#[derive(Serialize, Deserialize)]
struct SlotPayload {
    group: GroupId,
    epoch: EpochId,
    messages: Vec<Vec<u8>>,
}

/// What reading one slot did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SlotReport {
    pub messages: usize,
    /// Messages that were rejected (bad signature, removed author, ...).
    pub failed: usize,
    /// Slots that didn't decrypt or had a wrong header.
    pub bad_slots: usize,
}

fn slot_aad(group: &GroupId, epoch: &EpochId, topic: &[u8; 32]) -> Vec<u8> {
    let mut v = Vec::with_capacity(110);
    v.extend_from_slice(b"kt/slot/v1");
    v.extend_from_slice(&group.0);
    v.extend_from_slice(&epoch.0);
    v.extend_from_slice(topic);
    v
}

impl Replica {
    /// The slots this device should write now.
    pub fn slot_targets(&self) -> Vec<SlotTarget> {
        let now = super::now_ms();
        let me = self.identity.id();
        let mut out = Vec::new();
        for (g, state) in &self.groups {
            if !state.epochs.is_current_member(&me) {
                continue;
            }
            let mut push = |epoch: EpochId, role: SlotRole| {
                if let Some(key) = state.keys.get(&epoch) {
                    if out.iter().any(|t: &SlotTarget| t.group == *g && t.epoch == epoch) {
                        return;
                    }
                    let mut info = Vec::with_capacity(64);
                    info.extend_from_slice(&g.0);
                    info.extend_from_slice(&epoch.0);
                    let seed = crypto::derive(&self.identity.secret().to_bytes(), "kt/slot-writer/v1", &info);
                    out.push(SlotTarget { group: *g, epoch, topic: topic_for(key, &epoch), role, writer_seed: seed.0 });
                }
            };
            let heads = state.epochs.heads();
            for h in &heads {
                push(*h, SlotRole::Main);
            }
            for h in &heads {
                let Some(rec) = state.epochs.record(h) else { continue };
                if now.saturating_sub(rec.body.created_ms) < FORWARD_WINDOW_MS {
                    for p in &rec.body.parents {
                        push(*p, SlotRole::Forward);
                    }
                }
            }
        }
        out
    }

    /// The topics this device should read: every current head plus the most recent epochs
    /// it holds keys for (to catch forward slots and members that haven't switched yet).
    pub fn read_topics(&self) -> Vec<ReadTopic> {
        let mut out = Vec::new();
        for (g, state) in &self.groups {
            let mut epochs: Vec<(u64, EpochId)> = state
                .keys
                .keys()
                .map(|e| (state.epochs.record(e).map(|r| r.body.created_ms).unwrap_or(u64::MAX), *e))
                .collect();
            epochs.sort_by(|a, b| b.cmp(a));
            let mut chosen: Vec<EpochId> = state.epochs.heads().into_iter().filter(|h| state.keys.contains_key(h)).collect();
            for (_, e) in epochs {
                if chosen.len() >= READ_EPOCHS.max(state.epochs.heads().len()) {
                    break;
                }
                if !chosen.contains(&e) {
                    chosen.push(e);
                }
            }
            for e in chosen {
                out.push(ReadTopic { group: *g, epoch: e, topic: topic_for(&state.keys[&e], &e) });
            }
        }
        out
    }

    /// Build the sealed contents of a slot. Deterministic in content (not in bytes: the
    /// nonce is random), so callers compare [`Replica::slot_fingerprint`] to skip rewrites.
    pub fn slot_blob(&mut self, t: &SlotTarget) -> Result<Vec<u8>> {
        let messages = self.slot_messages(t)?;
        let key = self.groups[&t.group].keys.get(&t.epoch).cloned().ok_or_else(|| Error::NotFound("slot key".into()))?;
        let payload = wire::encode(SLOT_TAG, SLOT_VERSION, &SlotPayload { group: t.group, epoch: t.epoch, messages })?;
        Ok(crypto::seal(&key, &slot_aad(&t.group, &t.epoch, &t.topic), &payload))
    }

    /// A hash of what a slot would contain now (changes only when its content does).
    pub fn slot_fingerprint(&mut self, t: &SlotTarget) -> Result<[u8; 32]> {
        let mut h = Vec::new();
        for m in self.slot_messages_fingerprint_input(t)? {
            h.extend_from_slice(&crypto::hash("kt/slot-item/v1", &m));
        }
        Ok(crypto::hash("kt/slot-fp/v1", &h))
    }

    fn slot_messages_fingerprint_input(&mut self, t: &SlotTarget) -> Result<Vec<Vec<u8>>> {
        // Envelopes carry random nonces, so fingerprint the plaintext ingredients instead.
        let me_peer = self.identity.peer_id();
        let state = self.groups.get(&t.group).ok_or_else(|| Error::NotFound("group".into()))?;
        let mut out: Vec<Vec<u8>> = self.carried_records(&t.group)?.into_iter().collect();
        if t.role == SlotRole::Main {
            for (d, ds) in &state.docs {
                let cur = ds.doc.oplog_vv().get(&me_peer).copied().unwrap_or(0);
                let mut v = d.0.to_vec();
                v.extend_from_slice(&ds.generation.to_le_bytes());
                v.extend_from_slice(&cur.to_le_bytes());
                v.extend_from_slice(&ds.gen_base.to_le_bytes());
                out.push(v);
            }
            out.push(t.epoch.0.to_vec());
        }
        Ok(out)
    }

    /// Epoch records this device carries: its own, plus those whose creator is no longer a
    /// member (nobody else would republish them).
    fn carried_records(&self, g: &GroupId) -> Result<Vec<Vec<u8>>> {
        let state = &self.groups[g];
        let me = self.identity.id();
        let mut out = Vec::new();
        for rec in state.epochs.all_records() {
            let c = rec.body.creator;
            if c == me || !state.epochs.is_current_member(&c) {
                out.push(rec.encode()?);
            }
        }
        Ok(out)
    }

    fn slot_messages(&mut self, t: &SlotTarget) -> Result<Vec<Vec<u8>>> {
        let mut messages = Vec::new();
        for r in self.carried_records(&t.group)? {
            messages.push(Message::Epoch(r).encode()?);
        }
        if t.role == SlotRole::Forward {
            return Ok(messages);
        }
        let me = self.identity.id();
        let me_peer = self.identity.peer_id();
        let state = &self.groups[&t.group];
        // Envelopes need one agreed head (a forked key graph merges quickly, rule 2).
        if state.epochs.head() != Some(t.epoch) || !state.epochs.is_current_member(&me) {
            return Ok(messages);
        }
        let key = state.keys.get(&t.epoch).cloned().ok_or_else(|| Error::NotFound("head key".into()))?;
        let docs: Vec<_> = state.docs.keys().copied().collect();
        for d in docs {
            let ds = &self.groups[&t.group].docs[&d];
            // The generation's proposal (the document's base), if nobody else carries it.
            if ds.generation > 0 {
                if let Some(first) = crate::storage::read_records(&self.layout.doc_updates(&t.group, &d))?.into_iter().next() {
                    if let Ok(env) = Envelope::decode(&first) {
                        let carries = env.sender == me || !state.epochs.is_current_member(&env.sender);
                        if carries {
                            if let Some(k) = state.keys.get(&env.epoch) {
                                if env.open(&t.group, &state.epochs, k).map(|i| i.kind == MessageKind::DocEpochProposal).unwrap_or(false) {
                                    messages.push(Message::Envelope(first).encode()?);
                                }
                            }
                        }
                    }
                }
            }
            let cur = ds.doc.oplog_vv().get(&me_peer).copied().unwrap_or(0);
            if cur > ds.gen_base {
                let span = IdSpan::new(me_peer, ds.gen_base, cur);
                let update = ds.doc.export(ExportMode::updates_in_range(vec![span])).map_err(|e| Error::Loro(e.to_string()))?;
                let inner = Inner {
                    group: t.group,
                    doc: d,
                    generation: ds.generation,
                    kind: MessageKind::DocUpdate,
                    peer: me_peer,
                    created_ms: super::now_ms(),
                    payload: update,
                };
                messages.push(Message::Envelope(Envelope::seal(&self.identity, t.epoch, &key, &inner)?.encode()?).encode()?);
            }
        }
        Ok(messages)
    }

    /// Open one slot read from a mailbox and apply everything in it.
    pub fn open_slot(&mut self, t: &ReadTopic, blob: &[u8]) -> Result<SlotReport> {
        self.open_slots(t, &[blob])
    }

    /// Open all slots fetched for one topic and apply them. Epoch records from **every**
    /// slot go first, so a removal published by one member is known before a removed
    /// device's late edits in another slot are considered (the remaining cross-fetch race is
    /// closed by document epochs at removal cutoffs, D24). Untrusted input: each message is
    /// verified on its own, and one bad slot or message doesn't block the rest.
    pub fn open_slots(&mut self, t: &ReadTopic, blobs: &[&[u8]]) -> Result<SlotReport> {
        let key = self
            .groups
            .get(&t.group)
            .and_then(|s| s.keys.get(&t.epoch))
            .cloned()
            .ok_or_else(|| Error::NotFound("slot key".into()))?;
        let aad = slot_aad(&t.group, &t.epoch, &t.topic);
        let mut report = SlotReport::default();
        let mut epochs = Vec::new();
        let mut others = Vec::new();
        for blob in blobs {
            let payload: SlotPayload = match crypto::open(&key, &aad, blob)
                .map_err(Error::from)
                .and_then(|p| wire::decode(SLOT_TAG, SLOT_VERSION, &p).map(|x| x.1))
            {
                Ok(p) => p,
                Err(_) => {
                    report.bad_slots += 1;
                    continue;
                }
            };
            if payload.group != t.group || payload.epoch != t.epoch {
                report.bad_slots += 1;
                continue;
            }
            report.messages += payload.messages.len();
            for m in payload.messages {
                if matches!(Message::decode(&m), Ok(Message::Epoch(_))) {
                    epochs.push(m);
                } else {
                    others.push(m);
                }
            }
        }
        for m in epochs.iter().chain(others.iter()) {
            if self.receive(m).is_err() {
                report.failed += 1;
            }
        }
        Ok(report)
    }
}
