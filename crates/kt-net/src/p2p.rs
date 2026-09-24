//! Peer-to-peer sync over iroh (DESIGN §6, D49).
//!
//! - Each device runs one iroh endpoint. Its address (endpoint ID, relay URLs, IPs) is
//!   published to group members only, as signed whereabouts inside sealed slots.
//! - **Relays:** a custom relay map (helper relays, select relays; D49: IPv4 URLs), empty
//!   by default. n0's relays only if explicitly enabled (best-effort layer, D50).
//! - **Protocol** (ALPN `kt/slots/1`), one bidirectional stream per exchange:
//!   1. dialer → `Hello { topics, have }` (its read topics for one group + store summary)
//!   2. responder → `Reply { slots newer than have, its own have }`
//!   3. dialer → `Push { slots the responder lacks }`
//!   4. responder → `Ack { applied }` once it has applied them: delivery confirmation
//!      (the "On Alex's phone" state of D18), not just "sent".
//!
//!   Both sides then open what's new through the same path as mailbox slots (sealed,
//!   individually verified, sandboxed imports). Because the store holds every member's
//!   latest slot, a member device relays the others' changes ("relay through a member").
//! - Only member devices are served, and only for topics the responder itself reads: the
//!   endpoint must be listed in a member's whereabouts, or the dialer's `Hello` carries a
//!   device-signed whereabouts record naming this very endpoint (iroh authenticates
//!   endpoint IDs, so the proof can't be replayed from another endpoint).

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use iroh::endpoint::{presets, Connection, PortmapperConfig, RecvStream, SendStream};
use iroh::{Endpoint, EndpointAddr, EndpointId, RelayMap, RelayMode, RelayUrl, SecretKey, TransportAddr};
use kt_engine::GroupId;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::mailbox::SlotRead;
use crate::slotstore::Summary;
use crate::sync::{ReadReport, Syncer};

pub const ALPN: &[u8] = b"kt/slots/1";
const MAX_FRAME: usize = 32 * 1024 * 1024;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone, Debug, Default)]
pub struct P2pConfig {
    /// Custom relays (helper / select relays), e.g. "https://203.0.113.5".
    pub relays: Vec<String>,
    /// Also use n0's public relays (best-effort, D50).
    pub n0_relays: bool,
    /// Bind address; `None` = all interfaces on a random port. Tests use 127.0.0.1:0.
    pub bind: Option<SocketAddr>,
    /// Ask the router for a port mapping (UPnP / NAT-PMP / PCP).
    pub portmap: bool,
    /// Token presented to the custom relays (group-token access on helper relays).
    pub relay_token: Option<String>,
    /// No direct IP paths at all, only relays (tests, USB tunnels).
    pub relay_only: bool,
    /// Help other groups by storing their slots in RAM (§7a); `None` = not helping.
    pub helper: Option<crate::helper::HelperLimits>,
}

/// What a device publishes to its groups as whereabouts.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WhereData {
    pub endpoint: [u8; 32],
    pub relays: Vec<String>,
    pub ips: Vec<SocketAddr>,
}

impl WhereData {
    pub fn to_addr(&self) -> Option<EndpointAddr> {
        let id = EndpointId::from_bytes(&self.endpoint).ok()?;
        let mut addrs: Vec<TransportAddr> = self.ips.iter().map(|a| TransportAddr::Ip(*a)).collect();
        for r in &self.relays {
            if let Ok(u) = r.parse::<RelayUrl>() {
                addrs.push(TransportAddr::Relay(u));
            }
        }
        Some(EndpointAddr::from_parts(id, addrs))
    }
}

#[derive(Serialize, Deserialize)]
struct WireSlot {
    topic: [u8; 32],
    writer: [u8; 32],
    version: u64,
    blob: Vec<u8>,
}

impl From<SlotRead> for WireSlot {
    fn from(s: SlotRead) -> Self {
        WireSlot { topic: s.topic, writer: s.writer, version: s.version, blob: s.blob }
    }
}

impl From<WireSlot> for SlotRead {
    fn from(s: WireSlot) -> Self {
        SlotRead { topic: s.topic, writer: s.writer, version: s.version, blob: s.blob }
    }
}

#[derive(Serialize, Deserialize)]
struct Hello {
    topics: Vec<[u8; 32]>,
    have: Summary,
    /// The dialer's device-signed whereabouts for the group: proves that this endpoint
    /// belongs to a current member device, even if the responder hasn't heard of it yet.
    proof: Option<Vec<u8>>,
}

#[derive(Serialize, Deserialize)]
struct Reply {
    slots: Vec<WireSlot>,
    have: Summary,
}

#[derive(Serialize, Deserialize)]
struct Push {
    slots: Vec<WireSlot>,
}

#[derive(Serialize, Deserialize)]
struct Ack {
    applied: u32,
}

async fn send_frame<T: Serialize>(s: &mut SendStream, v: &T) -> Result<(), String> {
    let b = postcard::to_stdvec(v).map_err(|e| e.to_string())?;
    if b.len() > MAX_FRAME {
        return Err("frame too large".into());
    }
    s.write_all(&(b.len() as u32).to_le_bytes()).await.map_err(|e| e.to_string())?;
    s.write_all(&b).await.map_err(|e| e.to_string())
}

async fn recv_frame<T: DeserializeOwned>(r: &mut RecvStream) -> Result<T, String> {
    let mut len = [0u8; 4];
    r.read_exact(&mut len).await.map_err(|e| e.to_string())?;
    let n = u32::from_le_bytes(len) as usize;
    if n > MAX_FRAME {
        return Err("frame too large".into());
    }
    let mut b = vec![0u8; n];
    r.read_exact(&mut b).await.map_err(|e| e.to_string())?;
    postcard::from_bytes(&b).map_err(|e| e.to_string())
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct P2pReport {
    pub peers_tried: usize,
    pub peers_reached: usize,
    pub sent: usize,
    /// Slots the peers confirmed they applied.
    pub confirmed: usize,
    pub received: ReadReport,
}

pub struct P2p {
    ep: Endpoint,
    syncer: Arc<Syncer>,
    conns: Mutex<HashMap<(EndpointId, Vec<u8>), Connection>>,
    helper: Option<Arc<crate::helper::HelperStore>>,
}

impl P2p {
    /// Start the endpoint. Its key is derived from the device secret (one-way, so the
    /// network ID reveals nothing about the device key).
    pub async fn start(syncer: Arc<Syncer>, cfg: P2pConfig) -> Result<Arc<P2p>, String> {
        let seed = {
            let r = syncer.replica();
            let r = r.lock().expect("replica lock");
            kt_engine::crypto::derive(&r.identity().secret().to_bytes(), "kt/iroh-endpoint/v1", b"").0
        };
        let relay_mode = if cfg.n0_relays {
            RelayMode::Default
        } else {
            let mut cfgs = Vec::new();
            for r in &cfg.relays {
                let u: RelayUrl = r.parse().map_err(|e| format!("bad relay url {r}: {e}"))?;
                let mut c = iroh::RelayConfig::new(u, None);
                if let Some(t) = &cfg.relay_token {
                    c = c.with_auth_token(t);
                }
                cfgs.push(Arc::new(c));
            }
            // An empty custom map (not Disabled) so relays can be added at runtime (E10).
            RelayMode::Custom(RelayMap::from_iter(cfgs))
        };
        let mut b = Endpoint::builder(presets::Minimal)
            .secret_key(SecretKey::from_bytes(&seed))
            .alpns(vec![ALPN.to_vec(), crate::helper::HELPER_ALPN.to_vec()])
            .relay_mode(relay_mode);
        if !cfg.portmap {
            b = b.portmapper_config(PortmapperConfig::Disabled);
        }
        if cfg.relay_only {
            b = b.clear_ip_transports();
        } else if let Some(a) = cfg.bind {
            b = b.clear_ip_transports().bind_addr(a).map_err(|e| e.to_string())?;
        }
        let ep = b.bind().await.map_err(|e| e.to_string())?;
        let helper = cfg.helper.clone().map(crate::helper::HelperStore::new);
        let p = Arc::new(P2p { ep, syncer, conns: Mutex::new(HashMap::new()), helper });
        let acc = p.clone();
        tokio::spawn(async move { acc.accept_loop().await });
        Ok(p)
    }

    pub fn endpoint(&self) -> &Endpoint {
        &self.ep
    }

    pub fn where_data(&self) -> WhereData {
        let a = self.ep.addr();
        let mut relays = Vec::new();
        let mut ips = Vec::new();
        for t in a.addrs.iter() {
            match t {
                TransportAddr::Relay(u) => relays.push(u.to_string()),
                TransportAddr::Ip(s) => ips.push(*s),
                _ => {}
            }
        }
        WhereData { endpoint: *self.ep.id().as_bytes(), relays, ips }
    }

    /// Put our current address into our slots (members learn it on their next read).
    pub fn publish_whereabouts(&self) {
        let data = postcard::to_stdvec(&self.where_data()).ok();
        self.syncer.replica().lock().expect("replica lock").set_whereabouts(data);
    }

    /// Current members' endpoint addresses per group.
    fn members(&self) -> Vec<(GroupId, EndpointAddr)> {
        let r = self.syncer.replica();
        let r = r.lock().expect("replica lock");
        let mut out = Vec::new();
        for g in r.groups() {
            for w in r.member_whereabouts(&g) {
                if let Some(a) = postcard::from_bytes::<WhereData>(&w.data).ok().and_then(|d| d.to_addr()) {
                    out.push((g, a));
                }
            }
        }
        out
    }

    fn topics_of(&self, g: &GroupId) -> Vec<[u8; 32]> {
        let r = self.syncer.replica();
        let r = r.lock().expect("replica lock");
        r.read_topics().into_iter().filter(|t| t.group == *g).map(|t| t.topic).collect()
    }

    async fn connection(&self, addr: EndpointAddr) -> Result<Connection, String> {
        self.connection_alpn(addr, ALPN).await
    }

    /// A (cached) connection to `addr` for protocol `alpn`.
    pub(crate) async fn connection_alpn(&self, addr: EndpointAddr, alpn: &[u8]) -> Result<Connection, String> {
        let key = (addr.id, alpn.to_vec());
        if let Some(c) = self.conns.lock().expect("lock").get(&key) {
            if c.close_reason().is_none() {
                return Ok(c.clone());
            }
        }
        let c = tokio::time::timeout(CONNECT_TIMEOUT, self.ep.connect(addr, alpn))
            .await
            .map_err(|_| "connect timeout".to_string())?
            .map_err(|e| e.to_string())?;
        self.conns.lock().expect("lock").insert(key, c.clone());
        Ok(c)
    }

    /// Our helper store, if helping.
    pub fn helper(&self) -> Option<Arc<crate::helper::HelperStore>> {
        self.helper.clone()
    }

    /// Exchange slots with every reachable member device, all at once (an offline member
    /// costs one connect timeout in parallel, not one each).
    pub async fn sync_round(&self) -> P2pReport {
        // Make sure our own current slots are in the store.
        self.syncer.write_round().await;
        let jobs: Vec<(GroupId, EndpointAddr, Vec<[u8; 32]>)> =
            self.members().into_iter().map(|(g, a)| (g, a, self.topics_of(&g))).filter(|(_, _, t)| !t.is_empty()).collect();
        let mut report = P2pReport { peers_tried: jobs.len(), ..Default::default() };
        let results = futures_util::future::join_all(jobs.into_iter().map(|(g, a, t)| self.exchange(g, a, t))).await;
        for (sent, confirmed, rr) in results.into_iter().flatten() {
            report.peers_reached += 1;
            report.sent += sent;
            report.confirmed += confirmed;
            report.received.slots_opened += rr.slots_opened;
            report.received.messages += rr.messages;
            report.received.rejected += rr.rejected;
            report.received.already_seen += rr.already_seen;
        }
        report
    }

    async fn exchange(&self, g: GroupId, addr: EndpointAddr, topics: Vec<[u8; 32]>) -> Result<(usize, usize, ReadReport), String> {
        let conn = self.connection(addr).await?;
        let (mut s, mut r) = conn.open_bi().await.map_err(|e| e.to_string())?;
        let store = self.syncer.store();
        let proof = self.syncer.replica().lock().expect("replica lock").own_whereabouts_record(&g);
        send_frame(&mut s, &Hello { topics: topics.clone(), have: store.summary(&topics), proof }).await?;
        let reply: Reply = recv_frame(&mut r).await?;
        let push: Vec<WireSlot> = store.newer_than(&topics, &reply.have).into_iter().map(WireSlot::from).collect();
        let sent = push.len();
        send_frame(&mut s, &Push { slots: push }).await?;
        let _ = s.finish();
        let incoming: Vec<SlotRead> = reply.slots.into_iter().filter(|x| topics.contains(&x.topic)).map(SlotRead::from).collect();
        let rr = self.syncer.apply_incoming(incoming).await;
        let ack: Ack = recv_frame(&mut r).await?;
        Ok((sent, ack.applied as usize, rr))
    }

    async fn accept_loop(self: Arc<Self>) {
        while let Some(inc) = self.ep.accept().await {
            let me = self.clone();
            tokio::spawn(async move {
                let Ok(conn) = inc.await else { return };
                let remote = conn.remote_id();
                if conn.alpn() == crate::helper::HELPER_ALPN {
                    // Helping strangers: no membership needed, the store enforces its rules.
                    let Some(h) = me.helper.clone() else {
                        conn.close(2u32.into(), b"not helping");
                        return;
                    };
                    while let Ok((s, r)) = conn.accept_bi().await {
                        let h = h.clone();
                        let me3 = me.clone();
                        tokio::spawn(async move {
                            if let Ok(Some((put, fanout))) = h.serve(s, r).await {
                                crate::helper::forward(&me3, &put, &fanout).await;
                            }
                        });
                    }
                    return;
                }
                while let Ok((s, r)) = conn.accept_bi().await {
                    let me2 = me.clone();
                    tokio::spawn(async move {
                        let _ = me2.serve(remote, s, r).await;
                    });
                }
            });
        }
    }

    /// Groups whose members may use this connection: known member endpoints, or a valid
    /// membership proof for this very endpoint.
    fn allowed_topics(&self, remote: EndpointId, proof: Option<&[u8]>) -> Vec<[u8; 32]> {
        let mut groups: Vec<GroupId> = self.members().into_iter().filter(|(_, a)| a.id == remote).map(|(g, _)| g).collect();
        if let Some(p) = proof {
            let res = self.syncer.replica().lock().expect("replica lock").accept_whereabouts_record(p);
            if let Ok((g, _device, data)) = res {
                let bound = postcard::from_bytes::<WhereData>(&data).map(|w| w.endpoint == *remote.as_bytes()).unwrap_or(false);
                if bound && !groups.contains(&g) {
                    groups.push(g);
                }
            }
        }
        groups.iter().flat_map(|g| self.topics_of(g)).collect()
    }

    async fn serve(&self, remote: EndpointId, mut s: SendStream, mut r: RecvStream) -> Result<(), String> {
        let hello: Hello = recv_frame(&mut r).await?;
        let allowed = self.allowed_topics(remote, hello.proof.as_deref());
        if allowed.is_empty() {
            return Err("not a member".into());
        }
        let topics: Vec<[u8; 32]> = hello.topics.into_iter().filter(|t| allowed.contains(t)).collect();
        let store = self.syncer.store();
        let slots = store.newer_than(&topics, &hello.have).into_iter().map(WireSlot::from).collect();
        send_frame(&mut s, &Reply { slots, have: store.summary(&topics) }).await?;
        let push: Push = recv_frame(&mut r).await?;
        let incoming: Vec<SlotRead> = push.slots.into_iter().filter(|x| topics.contains(&x.topic)).map(SlotRead::from).collect();
        let n = incoming.len() as u32;
        let rr = self.syncer.apply_incoming(incoming).await;
        // Applied (or already had): new plus duplicates, minus nothing rejected per slot.
        let applied = n.min((rr.slots_opened + rr.already_seen) as u32);
        send_frame(&mut s, &Ack { applied }).await?;
        let _ = s.finish();
        Ok(())
    }

    /// Before going offline as a helper: pass every held slot to one other online helper of
    /// its set (§7a handoff). Returns how many slots found a new home.
    pub async fn handoff(&self) -> usize {
        let Some(h) = &self.helper else { return 0 };
        let mut moved = 0;
        for (put, set) in h.all() {
            for target in set.iter().filter(|r| r.endpoint != *self.ep.id().as_bytes()) {
                if crate::helper::forward(self, &put, std::slice::from_ref(target)).await > 0 {
                    moved += 1;
                    break;
                }
            }
        }
        moved
    }

    /// Close gracefully, but never hang on a peer that doesn't answer the close.
    pub async fn close(&self) {
        for c in self.conns.lock().expect("lock").drain().map(|(_, c)| c) {
            c.close(0u32.into(), b"bye");
        }
        let _ = tokio::time::timeout(Duration::from_secs(3), self.ep.close()).await;
    }
}
