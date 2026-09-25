//! Blossom (BUD-01/02) as a best-effort blob layer (DESIGN §4, D50): public HTTP servers
//! that keep blobs by SHA-256, the same ID ours have.
//!
//! - Only servers that accept `application/octet-stream` from any key are used; media-only
//!   servers are never fed ciphertext under a false type (probe 2026-09-25:
//!   `tests/blossom_probe.rs`).
//! - Every upload is authorized by a fresh random key, so uploads can't be linked to each
//!   other or to this device's other keys.
//! - Before re-offering, a HEAD tells whether the server still has it.
//! - A server that fails is skipped for a while; nothing here is ever required.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use base64::Engine as _;
use kt_engine::blobs::{blob_id, BlobId};
use nostr::prelude::*;

/// Seed servers (the probe's result). Best effort: the app works without any of them.
pub const SEED_SERVERS: [&str; 2] = ["https://nostr.download", "https://cdn.hzrd149.com"];
/// Largest blob offered (what the probe confirmed works).
pub const MAX_BLOB: usize = 3 * 1024 * 1024;
/// Servers each blob goes to.
pub const COPIES: usize = 2;
const SKIP_AFTER_FAILURE: Duration = Duration::from_secs(3600);

pub struct Blossom {
    servers: Vec<String>,
    client: reqwest::Client,
    down: Mutex<HashMap<String, Instant>>,
}

impl Blossom {
    pub fn new(servers: Vec<String>) -> Option<Blossom> {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let client = crate::http::client(Duration::from_secs(60))?;
        Some(Blossom { servers, client, down: Mutex::new(HashMap::new()) })
    }

    pub fn seeded() -> Option<Blossom> {
        Blossom::new(SEED_SERVERS.iter().map(|s| s.to_string()).collect())
    }

    fn up(&self) -> Vec<String> {
        let d = self.down.lock().expect("lock");
        self.servers.iter().filter(|s| d.get(*s).is_none_or(|t| t.elapsed() > SKIP_AFTER_FAILURE)).cloned().collect()
    }

    fn failed(&self, server: &str) {
        self.down.lock().expect("lock").insert(server.to_string(), Instant::now());
    }

    fn auth(verb: &str, sha: &str) -> Option<String> {
        let keys = Keys::generate(); // unlinkable: a new key per request
        let ev = EventBuilder::new(Kind::Custom(24242), format!("{verb} blob"))
            .tags([Tag::parse(["t", verb]).ok()?, Tag::parse(["x", sha]).ok()?, Tag::expiration(Timestamp::now() + 300)])
            .finalize(&keys)
            .ok()?;
        Some(format!("Nostr {}", base64::engine::general_purpose::STANDARD.encode(ev.as_json())))
    }

    /// Make sure up to [`COPIES`] servers hold the blob. Returns how many do.
    pub async fn offer(&self, data: &[u8]) -> usize {
        if data.len() > MAX_BLOB {
            return 0;
        }
        let sha = hex::encode(blob_id(data));
        let mut ok = 0;
        for s in self.up() {
            if ok >= COPIES {
                break;
            }
            // Already there: nothing to send.
            if matches!(self.client.head(format!("{s}/{sha}")).send().await, Ok(r) if r.status().is_success()) {
                ok += 1;
                continue;
            }
            let Some(auth) = Self::auth("upload", &sha) else { continue };
            match self.client.put(format!("{s}/upload")).header("Authorization", auth).header("Content-Type", "application/octet-stream").body(data.to_vec()).send().await {
                Ok(r) if r.status().is_success() => ok += 1,
                _ => self.failed(&s),
            }
        }
        ok
    }

    /// Fetch a blob; only the exact bytes are accepted.
    pub async fn get(&self, id: &BlobId) -> Option<Vec<u8>> {
        let sha = hex::encode(id);
        for s in self.up() {
            match self.client.get(format!("{s}/{sha}")).send().await {
                Ok(r) if r.status().is_success() => {
                    if let Ok(b) = r.bytes().await {
                        if b.len() <= MAX_BLOB + 1024 && blob_id(&b) == *id {
                            return Some(b.to_vec());
                        }
                    }
                    self.failed(&s); // served something else
                }
                Ok(_) => {} // not there: not a failure
                Err(_) => self.failed(&s),
            }
        }
        None
    }
}
