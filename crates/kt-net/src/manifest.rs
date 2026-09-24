//! Operator manifest (DESIGN §7b, D56): optional developer-provided capacity.
//!
//! - A signed file listing *additional* candidates: Nostr relays, iroh relays, helper
//!   nodes, each with a priority weight. There is no field that can disable or remove
//!   anything: the effective lists are always baseline ∪ manifest.
//! - Signed with a separate **offline** Ed25519 operator key; only its public key ships in
//!   the app. A compromised website, GitHub account or Nostr key can't forge it.
//! - Fetched from several sources (HTTPS URLs, Nostr); the newest valid version wins and
//!   the last good copy is kept on disk.
//! - Nostr: anyone can publish events with the manifest tag, but only a valid operator
//!   signature inside counts, so spam can't displace it (readers take the highest version).

use std::path::PathBuf;
use std::sync::Mutex;

use kt_engine::crypto;
use serde::{Deserialize, Serialize};

pub const MANIFEST_KIND: u16 = 35712;
pub const MANIFEST_TOPIC_TAG: &str = "kt-operator-manifest";
const TAG: [u8; 2] = *b"OM";
const MAX_MANIFEST: usize = 256 * 1024;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Weighted<T> {
    pub item: T,
    /// Higher = preferred. Select nodes still earn trust like any helper (§7a).
    pub weight: u16,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestBody {
    /// Monotonic; a lower version never replaces a higher one.
    pub version: u64,
    pub issued_ms: u64,
    pub nostr_relays: Vec<Weighted<String>>,
    pub iroh_relays: Vec<Weighted<String>>,
    /// Helper / select nodes by iroh endpoint ID.
    pub helpers: Vec<Weighted<[u8; 32]>>,
}

#[derive(Serialize, Deserialize)]
struct Signed {
    body: Vec<u8>,
    sig: Vec<u8>,
}

/// Sign a manifest (operator tooling only; the key never ships in the app).
pub fn sign(body: &ManifestBody, operator: &crypto::DeviceSecret) -> Vec<u8> {
    let b = kt_engine::wire::encode(TAG, 1, body).expect("encodable");
    let sig = operator.sign(&b).to_vec();
    postcard::to_stdvec(&Signed { body: b, sig }).expect("encodable")
}

/// Verify a manifest against the operator public key (untrusted input).
pub fn verify(bytes: &[u8], operator_pub: &[u8; 32]) -> Result<ManifestBody, String> {
    if bytes.len() > MAX_MANIFEST {
        return Err("manifest too large".into());
    }
    let s: Signed = postcard::from_bytes(bytes).map_err(|e| e.to_string())?;
    let sig: [u8; 64] = s.sig.as_slice().try_into().map_err(|_| "bad signature length".to_string())?;
    crypto::verify(operator_pub, &s.body, &sig).map_err(|e| e.to_string())?;
    Ok(kt_engine::wire::decode(TAG, 1, &s.body).map_err(|e| e.to_string())?.1)
}

/// The app's view: shipped baseline plus the newest valid manifest seen.
pub struct Manifest {
    operator_pub: [u8; 32],
    baseline: ManifestBody,
    current: Mutex<Option<(ManifestBody, Vec<u8>)>>,
    path: Option<PathBuf>,
}

impl Manifest {
    /// `path` keeps the last good copy across restarts.
    pub fn new(operator_pub: [u8; 32], baseline: ManifestBody, path: Option<PathBuf>) -> Manifest {
        let current = path
            .as_ref()
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|b| verify(&b, &operator_pub).ok().map(|m| (m, b)));
        Manifest { operator_pub, baseline, current: Mutex::new(current), path }
    }

    /// Consider a fetched manifest. Returns true if it became the current one.
    pub fn consider(&self, bytes: &[u8]) -> bool {
        let Ok(m) = verify(bytes, &self.operator_pub) else { return false };
        let mut cur = self.current.lock().expect("lock");
        let newer = cur.as_ref().is_none_or(|(c, _)| m.version > c.version) && m.version > self.baseline.version;
        if !newer {
            return false;
        }
        if let Some(p) = &self.path {
            let _ = kt_engine::storage::write_atomic(p, bytes);
        }
        *cur = Some((m, bytes.to_vec()));
        true
    }

    pub fn version(&self) -> u64 {
        self.current.lock().expect("lock").as_ref().map(|(m, _)| m.version).unwrap_or(self.baseline.version)
    }

    /// Baseline ∪ manifest, highest weight first, de-duplicated. Only ever adds.
    pub fn effective(&self) -> ManifestBody {
        let cur = self.current.lock().expect("lock").as_ref().map(|(m, _)| m.clone()).unwrap_or_default();
        fn merge<T: Clone + PartialEq>(a: &[Weighted<T>], b: &[Weighted<T>]) -> Vec<Weighted<T>> {
            let mut out: Vec<Weighted<T>> = Vec::new();
            for w in a.iter().chain(b.iter()) {
                match out.iter_mut().find(|x| x.item == w.item) {
                    Some(x) => x.weight = x.weight.max(w.weight),
                    None => out.push(w.clone()),
                }
            }
            out.sort_by(|x, y| y.weight.cmp(&x.weight));
            out
        }
        ManifestBody {
            version: cur.version.max(self.baseline.version),
            issued_ms: cur.issued_ms.max(self.baseline.issued_ms),
            nostr_relays: merge(&self.baseline.nostr_relays, &cur.nostr_relays),
            iroh_relays: merge(&self.baseline.iroh_relays, &cur.iroh_relays),
            helpers: merge(&self.baseline.helpers, &cur.helpers),
        }
    }

    /// Fetch from HTTPS sources; every source is tried, failures are ignored.
    pub async fn fetch_http(&self, urls: &[&str]) -> usize {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let Ok(client) = reqwest::Client::builder().timeout(std::time::Duration::from_secs(20)).build() else { return 0 };
        let mut accepted = 0;
        for u in urls {
            let Ok(resp) = client.get(*u).send().await else { continue };
            if !resp.status().is_success() {
                continue;
            }
            if let Ok(b) = resp.bytes().await {
                if self.consider(&b) {
                    accepted += 1;
                }
            }
        }
        accepted
    }

    /// Fetch from Nostr relays: events of [`MANIFEST_KIND`] tagged [`MANIFEST_TOPIC_TAG`],
    /// from any author (only the operator signature inside matters).
    pub async fn fetch_nostr(&self, relays: &[std::sync::Arc<crate::nostr::RelayConn>]) -> usize {
        use base64::Engine as _;
        use nostr::prelude::*;
        let filter = Filter::new().kind(Kind::Custom(MANIFEST_KIND)).hashtag(MANIFEST_TOPIC_TAG).limit(50);
        let mut accepted = 0;
        for r in relays {
            let Some(evs) = r.query(vec![filter.clone()]).await else { continue };
            for ev in evs {
                if ev.verify().is_err() {
                    continue;
                }
                if let Ok(b) = base64::engine::general_purpose::STANDARD.decode(&ev.content) {
                    if self.consider(&b) {
                        accepted += 1;
                    }
                }
            }
        }
        accepted
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn body(v: u64, relay: &str) -> ManifestBody {
        ManifestBody {
            version: v,
            issued_ms: v,
            nostr_relays: vec![Weighted { item: relay.into(), weight: 5 }],
            iroh_relays: vec![],
            helpers: vec![Weighted { item: [7; 32], weight: 9 }],
        }
    }

    #[test]
    fn only_newer_validly_signed_manifests_apply_and_they_only_add() {
        let op = crypto::DeviceSecret::generate();
        let other = crypto::DeviceSecret::generate();
        let baseline = body(1, "wss://base.example");
        let dir = std::env::temp_dir().join(format!("kt-manifest-{}", rand::random::<u32>()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("manifest.bin");
        let m = Manifest::new(op.public().sign, baseline, Some(path.clone()));
        assert!(!m.consider(&sign(&body(5, "wss://evil"), &other)), "wrong key");
        assert!(!m.consider(b"garbage"));
        assert!(m.consider(&sign(&body(3, "wss://new.example"), &op)));
        assert!(!m.consider(&sign(&body(2, "wss://older"), &op)), "no downgrade");
        let e = m.effective();
        let relays: Vec<_> = e.nostr_relays.iter().map(|w| w.item.as_str()).collect();
        assert!(relays.contains(&"wss://base.example") && relays.contains(&"wss://new.example"), "baseline kept: {relays:?}");
        // Last good copy survives a restart.
        let m2 = Manifest::new(op.public().sign, body(1, "wss://base.example"), Some(path));
        assert_eq!(m2.version(), 3);
        // Tampering breaks the signature.
        let mut t = sign(&body(9, "wss://x"), &op);
        let n = t.len();
        t[n / 2] ^= 1;
        assert!(!m2.consider(&t));
    }
}
