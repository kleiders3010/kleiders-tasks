//! Whereabouts beyond slots (DESIGN §6 discovery): records a transport can publish to
//! public lookup systems (the Mainline DHT) without revealing anything to non-members.
//!
//! - Record key: derived from the group's current epoch key and the device ID, so only
//!   members can compute where to look, and the key rotates with every key epoch.
//! - Value: the device-signed whereabouts record, sealed with an epoch-derived key. Other
//!   members could overwrite a record (they can derive the key) but can't forge its content.

use super::Replica;
use crate::crypto::{self, SymKey};
use crate::ids::{DeviceId, EpochId, GroupId};
use crate::{Error, Result};

/// Max sealed value size (BEP44 mutable items hold up to 1000 bytes).
pub const DHT_VALUE_MAX: usize = 1000;

/// A record this device should publish.
#[derive(Clone, Debug)]
pub struct DhtPublish {
    pub group: GroupId,
    /// Seed of the record's signing key (Ed25519), shared by the group's members.
    pub key_seed: [u8; 32],
    pub value: Vec<u8>,
}

/// A member record this device should look up.
#[derive(Clone, Debug)]
pub struct DhtLookup {
    pub group: GroupId,
    pub device: DeviceId,
    pub epoch: EpochId,
    pub key_seed: [u8; 32],
}

fn record_seed(epoch_key: &SymKey, epoch: &EpochId, device: &DeviceId) -> [u8; 32] {
    let mut info = Vec::with_capacity(64);
    info.extend_from_slice(&epoch.0);
    info.extend_from_slice(&device.0);
    crypto::derive(&epoch_key.0, "kt/dht-record/v1", &info).0
}

fn value_key(epoch_key: &SymKey, epoch: &EpochId) -> SymKey {
    crypto::derive(&epoch_key.0, "kt/dht-value/v1", &epoch.0)
}

fn value_aad(group_epoch: &EpochId, device: &DeviceId) -> Vec<u8> {
    let mut v = b"kt/dht/v1".to_vec();
    v.extend_from_slice(&group_epoch.0);
    v.extend_from_slice(&device.0);
    v
}

impl Replica {
    /// Our own records to publish (one per group we're a current member of).
    pub fn dht_publish(&self) -> Vec<DhtPublish> {
        let me = self.identity.id();
        let mut out = Vec::new();
        for (g, state) in &self.groups {
            let Some(head) = state.epochs.head() else { continue };
            // Nobody to find us in a group of one (a newly linked device makes it two).
            if !state.epochs.is_current_member(&me) || state.epochs.recipients(&head).is_none_or(|r| r.len() < 2) {
                continue;
            }
            let (Some(key), Ok(Some(rec))) = (state.keys.get(&head), self.whereabouts_record(g)) else { continue };
            let value = crypto::seal(&value_key(key, &head), &value_aad(&head, &me), &rec);
            if value.len() <= DHT_VALUE_MAX {
                out.push(DhtPublish { group: *g, key_seed: record_seed(key, &head, &me), value });
            }
        }
        out
    }

    /// Records of the other current members to look up.
    pub fn dht_lookups(&self) -> Vec<DhtLookup> {
        let me = self.identity.id();
        let mut out = Vec::new();
        for (g, state) in &self.groups {
            let Some(head) = state.epochs.head() else { continue };
            let (Some(key), Some(recipients)) = (state.keys.get(&head), state.epochs.recipients(&head)) else { continue };
            for d in recipients.keys().filter(|d| **d != me) {
                out.push(DhtLookup { group: *g, device: *d, epoch: head, key_seed: record_seed(key, &head, d) });
            }
        }
        out
    }

    /// Apply a value found for a lookup (untrusted: decrypted, then the inner record must
    /// be signed by that member device).
    pub fn open_dht_value(&mut self, l: &DhtLookup, value: &[u8]) -> Result<()> {
        let key = self
            .groups
            .get(&l.group)
            .and_then(|s| s.keys.get(&l.epoch))
            .cloned()
            .ok_or_else(|| Error::NotFound("epoch key".into()))?;
        let rec = crypto::open(&value_key(&key, &l.epoch), &value_aad(&l.epoch, &l.device), value)?;
        if !self.whereabouts_signed_by(&rec, &l.device) {
            return Err(Error::Rejected("whereabouts record not from that device".into()));
        }
        self.receive_whereabouts(&rec)
    }
}
