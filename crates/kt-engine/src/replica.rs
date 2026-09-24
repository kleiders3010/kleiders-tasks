//! One device's view of all its groups and documents: the engine's main entry point.
//!
//! Transport-agnostic (principle 4): the replica produces wire messages ([`Replica::outbox`])
//! and consumes them ([`Replica::receive`]) in any order, with duplicates. Transports (M2)
//! only move bytes.
//!
//! Receiving a document update:
//! 1. the envelope's epoch must be known (otherwise the message is parked);
//! 2. signature, membership, decryption and PeerID binding (see `envelope`);
//! 3. removal cutoffs (D5 rule 5);
//! 4. quarantine check, then the import sandbox validates against a copy of the live doc
//!    and proves the update touches only the sender's own PeerID (D2, D3);
//! 5. import into the live doc and persist the raw envelope (kept for relaying to members).

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::Path;

use loro::{ExportMode, IdSpan, LoroDoc};
use serde::{Deserialize, Serialize};

use crate::crypto::{DevicePublic, SymKey};
use crate::envelope::{Envelope, Inner, MessageKind};
use crate::epochs::{self, EpochRecord, GroupEpochs, InsertOutcome};
use crate::identity::{DeviceIdentity, IdentityLock};
use crate::ids::{peer_id_for, DeviceId, DocId, EpochId, GroupId};
use crate::sandbox::{self, Quarantine, SandboxConfig, Verdict};
use crate::storage::{self, Layout};
use crate::{crypto, wire, Error, Result};

const MSG_TAG: [u8; 2] = *b"MS";
const MSG_VERSION: u16 = 1;
/// Save a fresh snapshot after this many imported updates.
const SNAPSHOT_EVERY: usize = 64;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Message {
    Epoch(Vec<u8>),
    Envelope(Vec<u8>),
}

impl Message {
    pub fn encode(&self) -> Result<Vec<u8>> {
        wire::encode(MSG_TAG, MSG_VERSION, self)
    }
    pub fn decode(b: &[u8]) -> Result<Self> {
        Ok(wire::decode(MSG_TAG, MSG_VERSION, b)?.1)
    }
}

/// Things the host (UI, logs, tests) may want to know about.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    Imported { group: GroupId, doc: DocId, author: DeviceId },
    Rejected { reason: String },
    Quarantined { author: DeviceId },
    Parked,
    EpochAccepted { group: GroupId, epoch: EpochId },
}

struct DocState {
    doc: LoroDoc,
    /// Own ops already sent (counter end, exclusive). Resent after restart (idempotent).
    sent_counter: i32,
    imported_since_snapshot: usize,
}

struct GroupState {
    epochs: GroupEpochs,
    keys: HashMap<EpochId, SymKey>,
    docs: BTreeMap<DocId, DocState>,
    new_records: Vec<EpochRecord>,
}

pub struct Replica {
    identity: DeviceIdentity,
    _lock: IdentityLock,
    layout: Layout,
    sandbox: SandboxConfig,
    quarantine: Quarantine,
    groups: BTreeMap<GroupId, GroupState>,
    /// Groups we're allowed to learn about (created, joined or invited to).
    accepted_groups: BTreeSet<GroupId>,
    parked: Vec<Vec<u8>>,
    events: Vec<Event>,
}

impl Replica {
    /// Open (or create) a replica. `identity_dir` must be excluded from backups (D1).
    pub fn open(identity_dir: &Path, data_dir: &Path, sandbox: SandboxConfig) -> Result<Self> {
        let identity = DeviceIdentity::load_or_create(identity_dir)?;
        let lock = IdentityLock::acquire(identity_dir)?;
        let layout = Layout::new(data_dir);
        let (quarantine, newly) = Quarantine::open(&layout.quarantine_journal(), &layout.quarantine_list())?;
        let mut r = Replica {
            identity,
            _lock: lock,
            layout,
            sandbox,
            quarantine,
            groups: BTreeMap::new(),
            accepted_groups: BTreeSet::new(),
            parked: Vec::new(),
            events: newly.into_iter().map(|(_, a)| Event::Quarantined { author: DeviceId(a) }).collect(),
        };
        r.load()?;
        Ok(r)
    }

    fn load(&mut self) -> Result<()> {
        for g in self.layout.groups()? {
            self.accepted_groups.insert(g);
            let mut state = GroupState { epochs: GroupEpochs::new(g), keys: HashMap::new(), docs: BTreeMap::new(), new_records: Vec::new() };
            for rec in storage::read_records(&self.layout.control_log(&g))? {
                if let Ok(r) = EpochRecord::decode(&rec) {
                    let _ = state.epochs.insert(r);
                }
            }
            self.groups.insert(g, state);
            self.refresh_keys(&g);
            for d in self.layout.docs(&g)? {
                let doc = match std::fs::read(self.layout.doc_snapshot(&g, &d)) {
                    Ok(b) => LoroDoc::from_snapshot(&b).map_err(|e| Error::Loro(e.to_string()))?,
                    Err(_) => LoroDoc::new(),
                };
                doc.set_peer_id(self.identity.peer_id()).map_err(|e| Error::Loro(e.to_string()))?;
                self.groups.get_mut(&g).expect("inserted").docs.insert(d, DocState { doc, sent_counter: 0, imported_since_snapshot: 0 });
                // Re-apply logged envelopes (idempotent): restores pending changes too.
                for env in storage::read_records(&self.layout.doc_updates(&g, &d))? {
                    let _ = self.apply_envelope_bytes(&env, false);
                }
            }
        }
        Ok(())
    }

    pub fn identity(&self) -> &DeviceIdentity {
        &self.identity
    }

    pub fn device_id(&self) -> DeviceId {
        self.identity.id()
    }

    pub fn device_public(&self) -> DevicePublic {
        self.identity.public()
    }

    pub fn take_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }

    pub fn groups(&self) -> Vec<GroupId> {
        self.groups.keys().copied().collect()
    }

    pub fn epochs(&self, g: &GroupId) -> Option<&GroupEpochs> {
        self.groups.get(g).map(|s| &s.epochs)
    }

    pub fn docs(&self, g: &GroupId) -> Vec<DocId> {
        self.groups.get(g).map(|s| s.docs.keys().copied().collect()).unwrap_or_default()
    }

    /// Allow epoch records of this group to be accepted (after an invite, for example).
    pub fn accept_group(&mut self, g: GroupId) {
        self.accepted_groups.insert(g);
    }

    /// Make a key known out of band (the invite path, D55): lets a joiner read before being added.
    pub fn add_invite_key(&mut self, g: GroupId, epoch: EpochId, key: SymKey) {
        self.accept_group(g);
        let state = self.groups.entry(g).or_insert_with(|| GroupState {
            epochs: GroupEpochs::new(g),
            keys: HashMap::new(),
            docs: BTreeMap::new(),
            new_records: Vec::new(),
        });
        state.keys.insert(epoch, key);
    }

    pub fn create_group(&mut self) -> Result<GroupId> {
        let g = GroupId::random();
        let rec = epochs::genesis(&self.identity, g);
        self.accept_group(g);
        self.groups.insert(g, GroupState { epochs: GroupEpochs::new(g), keys: HashMap::new(), docs: BTreeMap::new(), new_records: Vec::new() });
        self.insert_epoch(g, rec, true)?;
        Ok(g)
    }

    pub fn create_doc(&mut self, g: &GroupId) -> Result<DocId> {
        let d = DocId::random();
        self.ensure_doc(g, &d)?;
        Ok(d)
    }

    fn ensure_doc(&mut self, g: &GroupId, d: &DocId) -> Result<()> {
        let peer = self.identity.peer_id();
        let state = self.groups.get_mut(g).ok_or_else(|| Error::NotFound("group".into()))?;
        if !state.docs.contains_key(d) {
            let doc = LoroDoc::new();
            doc.set_peer_id(peer).map_err(|e| Error::Loro(e.to_string()))?;
            state.docs.insert(*d, DocState { doc, sent_counter: 0, imported_since_snapshot: 0 });
            std::fs::create_dir_all(self.layout.doc_dir(g, d))?;
        }
        Ok(())
    }

    /// Read access to a document (for the domain layer).
    pub fn doc(&self, g: &GroupId, d: &DocId) -> Option<&LoroDoc> {
        self.groups.get(g)?.docs.get(d).map(|s| &s.doc)
    }

    /// Commit local edits made on [`Replica::doc`] and persist a snapshot.
    pub fn commit(&mut self, g: &GroupId, d: &DocId) -> Result<()> {
        let state = self.groups.get_mut(g).ok_or_else(|| Error::NotFound("group".into()))?;
        let ds = state.docs.get_mut(d).ok_or_else(|| Error::NotFound("doc".into()))?;
        ds.doc.commit();
        let snap = ds.doc.export(ExportMode::Snapshot).map_err(|e| Error::Loro(e.to_string()))?;
        storage::write_atomic(&self.layout.doc_snapshot(g, d), &snap)?;
        ds.imported_since_snapshot = 0;
        Ok(())
    }

    pub fn add_member(&mut self, g: &GroupId, device: DevicePublic) -> Result<()> {
        let rec = self.groups.get(g).ok_or_else(|| Error::NotFound("group".into()))?.epochs.create_add(&self.identity, device)?;
        self.insert_epoch(*g, rec, true)
    }

    pub fn remove_member(&mut self, g: &GroupId, device: DeviceId) -> Result<()> {
        let state = self.groups.get(g).ok_or_else(|| Error::NotFound("group".into()))?;
        let peer = peer_id_for(&device);
        let cutoffs = state.docs.iter().map(|(d, s)| (*d, s.doc.oplog_vv().get(&peer).copied().unwrap_or(0))).collect();
        let rec = state.epochs.create_remove(&self.identity, device, cutoffs)?;
        self.insert_epoch(*g, rec, true)
    }

    pub fn rotate(&mut self, g: &GroupId) -> Result<()> {
        let rec = self.groups.get(g).ok_or_else(|| Error::NotFound("group".into()))?.epochs.create_rotate(&self.identity)?;
        self.insert_epoch(*g, rec, true)
    }

    fn insert_epoch(&mut self, g: GroupId, rec: EpochRecord, own: bool) -> Result<()> {
        let bytes = rec.encode()?;
        let state = self.groups.get_mut(&g).ok_or_else(|| Error::NotFound("group".into()))?;
        let outcome = state.epochs.insert(rec.clone())?;
        if outcome == InsertOutcome::AlreadyKnown {
            return Ok(());
        }
        storage::append_record(&self.layout.control_log(&g), &bytes)?;
        if own {
            state.new_records.push(rec.clone());
        }
        if outcome == InsertOutcome::Accepted {
            self.events.push(Event::EpochAccepted { group: g, epoch: rec.id() });
        }
        self.refresh_keys(&g);
        // Deterministic merger duty (rule 2).
        let merge = self.groups[&g].epochs.merge_if_mine(&self.identity);
        if let Some(m) = merge {
            self.insert_epoch(g, m, true)?;
        }
        self.retry_parked();
        Ok(())
    }

    fn refresh_keys(&mut self, g: &GroupId) {
        let me = &self.identity;
        if let Some(state) = self.groups.get_mut(g) {
            let ids: Vec<EpochId> = state.epochs.all_records().map(|r| r.id()).collect();
            for id in ids {
                if !state.keys.contains_key(&id) {
                    if let Ok(k) = state.epochs.key_for(&id, me) {
                        state.keys.insert(id, k);
                    }
                }
            }
        }
    }

    fn retry_parked(&mut self) {
        let parked = std::mem::take(&mut self.parked);
        for p in parked {
            let _ = self.apply_envelope_bytes(&p, true);
        }
    }

    /// Everything this device should send now: its new epoch records and its own new ops.
    pub fn outbox(&mut self) -> Result<Vec<Vec<u8>>> {
        let mut out = Vec::new();
        let me_peer = self.identity.peer_id();
        let gids: Vec<GroupId> = self.groups.keys().copied().collect();
        for g in gids {
            let state = self.groups.get_mut(&g).expect("exists");
            for rec in state.new_records.drain(..) {
                out.push(Message::Epoch(rec.encode()?).encode()?);
            }
            let Some(head) = state.epochs.head() else { continue };
            if !state.epochs.is_current_member(&self.identity.id()) {
                continue;
            }
            let Some(key) = state.keys.get(&head).cloned() else { continue };
            let mut envs = Vec::new();
            for (d, ds) in state.docs.iter_mut() {
                let current = ds.doc.oplog_vv().get(&me_peer).copied().unwrap_or(0);
                if current <= ds.sent_counter {
                    continue;
                }
                let span = IdSpan::new(me_peer, ds.sent_counter, current);
                let update = ds.doc.export(ExportMode::updates_in_range(vec![span])).map_err(|e| Error::Loro(e.to_string()))?;
                let inner = Inner { group: g, doc: *d, kind: MessageKind::DocUpdate, peer: me_peer, created_ms: now_ms(), payload: update };
                let env = Envelope::seal(&self.identity, head, &key, &inner)?;
                let env_bytes = env.encode()?;
                ds.sent_counter = current;
                envs.push((*d, env_bytes));
            }
            for (d, env_bytes) in envs {
                storage::append_record(&self.layout.doc_updates(&g, &d), &env_bytes)?;
                out.push(Message::Envelope(env_bytes).encode()?);
            }
        }
        Ok(out)
    }

    /// Everything a new member of `g` needs to catch up: all epoch records and all stored
    /// envelopes (own and relayed, each still signed by its author).
    pub fn export_group(&self, g: &GroupId) -> Result<Vec<Vec<u8>>> {
        let mut out = Vec::new();
        for rec in storage::read_records(&self.layout.control_log(g))? {
            out.push(Message::Epoch(rec).encode()?);
        }
        for d in self.layout.docs(g)? {
            for env in storage::read_records(&self.layout.doc_updates(g, &d))? {
                out.push(Message::Envelope(env).encode()?);
            }
        }
        Ok(out)
    }

    /// Receive one wire message from any transport.
    pub fn receive(&mut self, bytes: &[u8]) -> Result<()> {
        match Message::decode(bytes)? {
            Message::Epoch(b) => {
                let rec = EpochRecord::decode(&b)?;
                let g = rec.body.group;
                if !self.accepted_groups.contains(&g) {
                    return Err(Error::Rejected("epoch for a group we didn't join".into()));
                }
                self.groups.entry(g).or_insert_with(|| GroupState {
                    epochs: GroupEpochs::new(g),
                    keys: HashMap::new(),
                    docs: BTreeMap::new(),
                    new_records: Vec::new(),
                });
                std::fs::create_dir_all(self.layout.group_dir(&g))?;
                self.insert_epoch(g, rec, false)
            }
            Message::Envelope(b) => self.apply_envelope_bytes(&b, true),
        }
    }

    fn find_group_of_epoch(&self, epoch: &EpochId) -> Option<GroupId> {
        self.groups.iter().find(|(_, s)| s.epochs.record(epoch).is_some() || s.keys.contains_key(epoch)).map(|(g, _)| *g)
    }

    fn apply_envelope_bytes(&mut self, bytes: &[u8], persist: bool) -> Result<()> {
        let env = Envelope::decode(bytes)?;
        let Some(g) = self.find_group_of_epoch(&env.epoch) else {
            if persist && self.parked.len() < 10_000 {
                self.parked.push(bytes.to_vec());
                self.events.push(Event::Parked);
            }
            return Ok(());
        };
        if env.sender == self.identity.id() && persist {
            return Ok(()); // our own message echoed back
        }
        let state = self.groups.get(&g).expect("found");
        if state.epochs.record(&env.epoch).is_none() {
            // Only an invite key so far: park until the epoch record arrives.
            if persist && self.parked.len() < 10_000 {
                self.parked.push(bytes.to_vec());
                self.events.push(Event::Parked);
            }
            return Ok(());
        }
        let key = state.keys.get(&env.epoch).cloned().ok_or_else(|| Error::Rejected("no key for that epoch".into()))?;
        let inner = match env.open(&g, &state.epochs, &key) {
            Ok(i) => i,
            Err(e) => {
                self.events.push(Event::Rejected { reason: e.to_string() });
                return Err(e);
            }
        };
        if inner.kind != MessageKind::DocUpdate {
            return Err(Error::Invalid("document epochs are handled by the domain layer".into()));
        }
        // D5 rule 5: a removed device's ops beyond its cutoff are rejected.
        if let Some(cutoffs) = state.epochs.cutoffs_for(&env.sender) {
            let end = LoroDoc::decode_import_blob_meta(&inner.payload, true)
                .map_err(|e| Error::Loro(e.to_string()))?
                .partial_end_vv
                .get(&inner.peer)
                .copied()
                .unwrap_or(0);
            let limit = cutoffs.get(&inner.doc).copied().unwrap_or(0);
            if end > limit {
                self.events.push(Event::Rejected { reason: "ops from a removed device beyond its cutoff".into() });
                return Err(Error::Rejected("removed device's late ops".into()));
            }
        }
        self.ensure_doc(&g, &inner.doc)?;
        let payload_hash = crypto::hash("kt/payload/v1", &inner.payload);
        if self.quarantine.is_quarantined(&payload_hash) {
            return Err(Error::Rejected("quarantined update".into()));
        }
        let snapshot = {
            let ds = &self.groups[&g].docs[&inner.doc];
            ds.doc.export(ExportMode::Snapshot).map_err(|e| Error::Loro(e.to_string()))?
        };
        self.quarantine.begin(payload_hash, env.sender.0)?;
        let verdict = sandbox::validate(&self.sandbox, &snapshot, &inner.payload);
        self.quarantine.end()?;
        match verdict? {
            Verdict::Ok { advanced_peers } => {
                if advanced_peers.iter().any(|p| *p != inner.peer) {
                    self.events.push(Event::Rejected { reason: "update contains another device's ops (impersonation)".into() });
                    return Err(Error::Rejected("impersonation".into()));
                }
            }
            Verdict::Rejected(reason) => {
                self.events.push(Event::Rejected { reason: reason.clone() });
                return Err(Error::Rejected(reason));
            }
            Verdict::Crashed(reason) => {
                self.quarantine.add(payload_hash, env.sender.0)?;
                self.events.push(Event::Quarantined { author: env.sender });
                return Err(Error::Rejected(format!("poison update quarantined: {reason}")));
            }
        }
        // Safe to import into the live document.
        self.quarantine.begin(payload_hash, env.sender.0)?;
        let ds = self.groups.get_mut(&g).expect("found").docs.get_mut(&inner.doc).expect("ensured");
        let res = ds.doc.import(&inner.payload);
        self.quarantine.end()?;
        res.map_err(|e| Error::Loro(e.to_string()))?;
        ds.imported_since_snapshot += 1;
        let snapshot_due = ds.imported_since_snapshot >= SNAPSHOT_EVERY;
        if persist {
            storage::append_record(&self.layout.doc_updates(&g, &inner.doc), bytes)?;
        }
        if snapshot_due {
            self.commit(&g, &inner.doc)?;
        }
        self.events.push(Event::Imported { group: g, doc: inner.doc, author: env.sender });
        Ok(())
    }
}

fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}
