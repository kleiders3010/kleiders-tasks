//! Discovery over the Mainline DHT (DESIGN §6): members publish sealed whereabouts as
//! BEP44 mutable items, in client mode, directly (not through n0 or any pkarr relay).
//!
//! Keys and values come from the engine (`Replica::dht_publish` / `dht_lookups`): only
//! group members can compute where a member's record lives, the location rotates with
//! every key epoch, and the content is sealed and device-signed.

use std::net::Ipv4Addr;
use std::sync::{Arc, Mutex};

use kt_engine::replica::Replica;
use mainline::async_dht::AsyncDht;
use mainline::{Dht, MutableItem, SigningKey};

pub struct DhtDiscovery {
    dht: AsyncDht,
    replica: Arc<Mutex<Replica>>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DhtReport {
    pub published: usize,
    pub publish_failed: usize,
    pub found: usize,
    pub applied: usize,
}

impl DhtDiscovery {
    /// `bootstrap`: `None` = the public Mainline DHT. `bind`: `None` = all interfaces.
    pub fn new(replica: Arc<Mutex<Replica>>, bootstrap: Option<Vec<String>>, bind: Option<Ipv4Addr>) -> std::io::Result<DhtDiscovery> {
        let mut b = Dht::builder();
        if let Some(bs) = &bootstrap {
            b.bootstrap(bs);
        }
        if let Some(ip) = bind {
            b.bind_address(ip);
        }
        Ok(DhtDiscovery { dht: b.build()?.as_async(), replica })
    }

    pub async fn bootstrapped(&self) -> bool {
        self.dht.bootstrapped().await
    }

    /// Publish our records (call when whereabouts change, and hourly: DHT items expire
    /// after about two hours).
    pub async fn publish(&self) -> DhtReport {
        let items = self.replica.lock().expect("replica lock").dht_publish();
        let seq = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
        let mut report = DhtReport::default();
        for it in items {
            let item = MutableItem::new(SigningKey::from_bytes(&it.key_seed), &it.value, seq, None);
            match self.dht.put_mutable(item, None).await {
                Ok(_) => report.published += 1,
                Err(_) => report.publish_failed += 1,
            }
        }
        report
    }

    /// Look up the other members' records and apply what's found.
    pub async fn lookup(&self) -> DhtReport {
        let lookups = self.replica.lock().expect("replica lock").dht_lookups();
        let mut report = DhtReport::default();
        for l in lookups {
            let pk = SigningKey::from_bytes(&l.key_seed).verifying_key().to_bytes();
            if let Some(item) = self.dht.get_mutable_most_recent(&pk, None).await {
                report.found += 1;
                if self.replica.lock().expect("replica lock").open_dht_value(&l, item.value()).is_ok() {
                    report.applied += 1;
                }
            }
        }
        report
    }
}
