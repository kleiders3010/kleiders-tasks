//! One iroh endpoint for the NAT matrix.
//!
//! Endpoint options:
//!   --seed N                deterministic secret key [N;32]
//!   --relay URL (repeat)    custom RelayMap with ONLY these relays (default: RelayMode::Disabled)
//!   --empty-relay-map       RelayMode::Custom(empty) instead of Disabled
//!   --relay-quic-port 7842  QAD port of the relays
//!   --ca PEM                trust these roots for relay TLS (Pebble root or self-signed CA)
//!   --no-portmap            PortmapperConfig::Disabled (default: enabled, as on desktop)
//!   --bind-port P           bind 0.0.0.0:P and [::]:P instead of random ports
//!   --ext ADDR (repeat)     Builder::external_addr (our own signaling knows this address)
//!   --stun ip:port (x2)     side-socket STUN probe before publishing
//!   --stun-predict          add_external_addr(stun public ip : iroh's bound v4 port)
//!   --relay-only            clear_ip_transports (relay-only endpoint)
//!   --ct-relay ip:port      option (c): add the TCP blind-relay custom transport (ct.rs)
//! Flow options:
//!   --settle MS             wait before publishing our address (default 3000)
//!   --out FILE              publish our EndpointAddr (JSON) = "our own signaling"
//!   --dial --peer-file FILE dial the peer from its published address
//!   --no-peer-relay / --no-peer-ips / --peer-ip-family 4|6   filter what we dial
//!   --go-file FILE          wait until the unix-ms time in FILE (simultaneous dial)
//!   --timeout S --observe S --bytes N --lifetime S
use crate::{args::Args, stun, util::*};
use iroh::{
    Endpoint, EndpointAddr, EndpointId, RelayConfig, RelayMap, RelayMode, RelayUrl, SecretKey,
    TransportAddr, Watcher,
    endpoint::{Connection, PathList, PortmapperConfig, presets},
};
use iroh_relay::{RelayQuicConfig, tls::CaTlsConfig};
use n0_future::StreamExt;
use serde_json::{Value, json};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const ALPN: &[u8] = b"kleiders/natlab/0";

pub fn relay_cfg(url: &RelayUrl, qp: u16) -> Arc<RelayConfig> {
    Arc::new(RelayConfig::new(url.clone(), Some(RelayQuicConfig::new(qp))))
}

pub async fn build_endpoint(a: &Args) -> anyhow::Result<Endpoint> {
    let seed: u8 = a.or("seed", 1);
    let sk = SecretKey::from_bytes(&[seed; 32]);
    let me = sk.public();
    let mut b = Endpoint::builder(presets::Minimal).secret_key(sk).alpns(vec![ALPN.to_vec()]);
    if let Some(ct) = a.get("ct-relay") {
        // option (c): custom transport as an extra (first) path
        b = b.add_custom_transport(crate::ct::TcpRelayTransport::connect(ct.parse()?, me).await?);
    }
    let relays = a.all("relay");
    let qp: u16 = a.or("relay-quic-port", 7842);
    if !relays.is_empty() {
        let mut cfgs = Vec::new();
        for r in &relays {
            let u: RelayUrl = r.parse()?;
            let mut c = relay_cfg(&u, qp);
            if let Some(t) = a.get("relay-token") {
                c = Arc::new(c.as_ref().clone().with_auth_token(t));
            }
            cfgs.push(c);
        }
        b = b.relay_mode(RelayMode::Custom(RelayMap::from_iter(cfgs)));
    } else if a.flag("empty-relay-map") {
        b = b.relay_mode(RelayMode::Custom(RelayMap::empty()));
    } else {
        b = b.relay_mode(RelayMode::Disabled);
    }
    if let Some(ca) = a.get("ca") {
        b = b.ca_tls_config(CaTlsConfig::custom_roots(load_certs(ca)?));
    }
    if a.flag("no-portmap") {
        b = b.portmapper_config(PortmapperConfig::Disabled);
    }
    if let Some(p) = a.get("bind-port") {
        b = b.bind_addr(format!("0.0.0.0:{p}"))?;
        if !a.flag("no-v6") {
            b = b.bind_addr(format!("[::]:{p}"))?;
        }
    }
    for e in a.all("ext") {
        b = b.external_addr(e.parse::<SocketAddr>()?);
    }
    if a.flag("relay-only") {
        b = b.clear_ip_transports();
    }
    Ok(b.bind().await?)
}

pub fn addr_json(a: &EndpointAddr) -> Value {
    let mut relays = Vec::new();
    let mut ips = Vec::new();
    let mut customs = Vec::new();
    for t in a.addrs.iter() {
        match t {
            TransportAddr::Relay(u) => relays.push(u.to_string()),
            TransportAddr::Ip(s) => ips.push(s.to_string()),
            TransportAddr::Custom(c) => customs.push(c.to_string()),
            _ => {}
        }
    }
    json!({"id": a.id.to_string(), "relays": relays, "ips": ips, "customs": customs})
}

pub fn parse_addr(v: &Value) -> anyhow::Result<EndpointAddr> {
    let id: EndpointId = v["id"].as_str().unwrap_or_default().parse()?;
    let mut addrs = Vec::new();
    for r in v["relays"].as_array().into_iter().flatten() {
        addrs.push(TransportAddr::Relay(r.as_str().unwrap_or_default().parse()?));
    }
    for i in v["ips"].as_array().into_iter().flatten() {
        addrs.push(TransportAddr::Ip(i.as_str().unwrap_or_default().parse()?));
    }
    for c in v["customs"].as_array().into_iter().flatten() {
        addrs.push(TransportAddr::Custom(c.as_str().unwrap_or_default().parse()?));
    }
    Ok(EndpointAddr::from_parts(id, addrs))
}

pub fn paths_json(pl: &PathList<'_>) -> Value {
    Value::Array(
        pl.iter()
            .map(|p| {
                json!({"id": format!("{:?}", p.id()), "relay": p.is_relay(),
                       "remote": format!("{:?}", p.remote_addr()),
                       "sel": p.is_selected(), "rtt_ms": p.rtt().as_secs_f64() * 1000.0})
            })
            .collect(),
    )
}

fn selected_is_ip(pl: &PathList<'_>) -> Option<bool> {
    pl.iter().find(|p| p.is_selected()).map(|p| p.is_ip())
}

pub fn spawn_watchers(ep: &Endpoint) {
    let s = ep.watch_addr().stream();
    tokio::spawn(async move {
        let mut s = std::pin::pin!(s);
        while let Some(a) = s.next().await {
            ev("addr", addr_json(&a));
        }
    });
    let s = ep.net_report().stream();
    tokio::spawn(async move {
        let mut s = std::pin::pin!(s);
        while let Some(r) = s.next().await {
            if let Some(r) = r {
                ev(
                    "netreport",
                    json!({"udp_v4": r.udp_v4, "udp_v6": r.udp_v6,
                    "global_v4": r.global_v4.map(|x| x.to_string()),
                    "global_v6": r.global_v6.map(|x| x.to_string()),
                    "varies_v4": r.mapping_varies_by_dest_ipv4,
                    "preferred_relay": r.preferred_relay.as_ref().map(|x| x.to_string()),
                    "captive": r.captive_portal}),
                );
            }
        }
    });
    let s = ep.home_relay_status().stream();
    tokio::spawn(async move {
        let mut s = std::pin::pin!(s);
        while let Some(v) = s.next().await {
            let st: Vec<Value> = v
                .iter()
                .map(|r| json!({"url": r.url().to_string(), "connected": r.is_connected(),
                              "err": r.last_error().map(|e| format!("{e:#}")),
                              "auth_denied": r.auth_denied_reason().map(|s| s.to_string())}))
                .collect();
            ev("relay_status", json!({"relays": st}));
        }
    });
}

async fn handle_stream(mut s: iroh::endpoint::SendStream, mut r: iroh::endpoint::RecvStream) {
    let Ok(data) = r.read_to_end(1 << 30).await else { return };
    if data.len() <= 64 {
        let _ = s.write_all(&data).await;
    } else {
        let _ = s.write_all(&(data.len() as u64).to_be_bytes()).await;
    }
    let _ = s.finish();
}

pub fn spawn_path_logger(conn: Connection, label: &'static str, t0: Instant, direct_at: Arc<Mutex<Option<f64>>>) {
    tokio::spawn(async move {
        let st = conn.paths_stream();
        let mut st = std::pin::pin!(st);
        while let Some(pl) = st.next().await {
            let el = t0.elapsed().as_secs_f64() * 1000.0;
            if selected_is_ip(&pl) == Some(true) {
                let mut d = direct_at.lock().unwrap();
                if d.is_none() {
                    *d = Some(el);
                }
            }
            ev(label, json!({"since_ms": el, "paths": paths_json(&pl)}));
        }
    });
}

pub fn spawn_accept(ep: Endpoint) {
    tokio::spawn(async move {
        while let Some(inc) = ep.accept().await {
            let ra = format!("{:?}", inc.remote_addr());
            tokio::spawn(async move {
                let t0 = Instant::now();
                match inc.await {
                    Ok(conn) => {
                        ev("incoming", json!({"remote_addr": ra, "remote_id": conn.remote_id().to_string(),
                                              "paths": paths_json(&conn.paths())}));
                        spawn_path_logger(conn.clone(), "in_paths", t0, Default::default());
                        while let Ok((s, r)) = conn.accept_bi().await {
                            tokio::spawn(handle_stream(s, r));
                        }
                    }
                    Err(e) => ev("incoming_fail", json!({"remote_addr": ra, "err": format!("{e:#}")})),
                }
            });
        }
    });
}

async fn echo1(conn: &Connection) -> anyhow::Result<()> {
    let (mut s, mut r) = conn.open_bi().await?;
    s.write_all(&[42]).await?;
    s.finish()?;
    let back = r.read_to_end(64).await?;
    anyhow::ensure!(back == [42]);
    Ok(())
}

async fn throughput(conn: &Connection, bytes: usize) -> anyhow::Result<f64> {
    let t = Instant::now();
    let (mut s, mut r) = conn.open_bi().await?;
    let chunk = vec![7u8; 64 * 1024];
    let mut left = bytes;
    while left > 0 {
        let n = left.min(chunk.len());
        s.write_all(&chunk[..n]).await?;
        left -= n;
    }
    s.finish()?;
    let back = r.read_to_end(16).await?;
    anyhow::ensure!(back.len() == 8);
    Ok(bytes as f64 * 8.0 / t.elapsed().as_secs_f64() / 1e6)
}

pub async fn wait_file(path: &str, secs: u64) -> anyhow::Result<String> {
    let t = Instant::now();
    loop {
        if let Ok(s) = std::fs::read_to_string(path) {
            if !s.trim().is_empty() {
                return Ok(s);
            }
        }
        anyhow::ensure!(t.elapsed().as_secs() < secs, "timeout waiting for {path}");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

pub fn write_atomic(path: &str, s: &str) -> anyhow::Result<()> {
    let tmp = format!("{path}.tmp");
    std::fs::write(&tmp, s)?;
    std::fs::rename(tmp, path)?;
    Ok(())
}

pub async fn dial(ep: &Endpoint, peer: EndpointAddr, a: &Args) -> Value {
    let t0 = Instant::now();
    let to = Duration::from_secs(a.or("timeout", 20));
    ev("dial_start", json!({"peer": addr_json(&peer)}));
    let conn = match tokio::time::timeout(to, ep.connect(peer, ALPN)).await {
        Err(_) => return json!({"connected": false, "err": "timeout", "ms": t0.elapsed().as_millis()}),
        Ok(Err(e)) => return json!({"connected": false, "err": format!("{e:#}"), "ms": t0.elapsed().as_millis()}),
        Ok(Ok(c)) => c,
    };
    let connect_ms = t0.elapsed().as_secs_f64() * 1000.0;
    let first_sel_ip = selected_is_ip(&conn.paths());
    ev("connected", json!({"ms": connect_ms, "paths": paths_json(&conn.paths())}));
    let direct_at: Arc<Mutex<Option<f64>>> = Default::default();
    if first_sel_ip == Some(true) {
        *direct_at.lock().unwrap() = Some(connect_ms);
    }
    spawn_path_logger(conn.clone(), "paths", t0, direct_at.clone());
    let ttfb = match tokio::time::timeout(to, echo1(&conn)).await {
        Ok(Ok(())) => Some(t0.elapsed().as_secs_f64() * 1000.0),
        _ => None,
    };
    ev("ttfb", json!({"ms": ttfb}));
    tokio::time::sleep(Duration::from_secs(a.or("observe", 10))).await;
    let bytes: usize = a.or("bytes", 0);
    let mbps = if bytes > 0 {
        tokio::time::timeout(Duration::from_secs(60), throughput(&conn, bytes)).await.ok().and_then(|r| r.ok())
    } else {
        None
    };
    let pl = conn.paths();
    let sel = pl.iter().find(|p| p.is_selected()).map(|p| {
        json!({"relay": p.is_relay(), "remote": format!("{:?}", p.remote_addr()), "rtt_ms": p.rtt().as_secs_f64()*1000.0})
    });
    let direct_ms = *direct_at.lock().unwrap();
    let alive = echo1(&conn).await.is_ok();
    json!({"connected": true, "connect_ms": connect_ms, "first_path_direct": first_sel_ip,
           "ttfb_ms": ttfb, "direct": direct_ms.is_some(), "direct_ms": direct_ms,
           "final_selected": sel, "n_paths": pl.len(), "mbps": mbps, "alive_after": alive})
}

fn filter_peer(mut addr: EndpointAddr, a: &Args) -> EndpointAddr {
    let fam: u8 = a.or("peer-ip-family", 0);
    let no_relay = a.flag("no-peer-relay");
    let no_ips = a.flag("no-peer-ips");
    addr.addrs.retain(|t| match t {
        TransportAddr::Relay(_) => !no_relay,
        TransportAddr::Ip(s) => !no_ips && (fam == 0 || (fam == 4 && s.is_ipv4()) || (fam == 6 && s.is_ipv6())),
        TransportAddr::Custom(_) => !a.flag("no-peer-custom"),
        _ => true,
    });
    // Endpoint::addr() never contains custom-transport addresses (watch_addr merges only IP +
    // relay), so "our signaling" adds the peer's custom address explicitly.
    if a.get("ct-relay").is_some() && !a.flag("no-peer-custom") {
        addr.addrs.insert(TransportAddr::Custom(crate::ct::to_addr(&addr.id)));
    }
    for extra in a.all("peer-extra-ip") {
        if let Ok(s) = extra.parse::<SocketAddr>() {
            addr.addrs.insert(TransportAddr::Ip(s));
        }
    }
    addr
}

pub async fn run(a: &Args) -> anyhow::Result<()> {
    let ep = build_endpoint(a).await?;
    ev("bound", json!({"id": ep.id().to_string(),
        "sockets": ep.bound_sockets().iter().map(|s| s.to_string()).collect::<Vec<_>>()}));
    spawn_watchers(&ep);
    spawn_accept(ep.clone());
    if !a.all("relay").is_empty() {
        let t = Instant::now();
        match tokio::time::timeout(Duration::from_secs(a.or("online-timeout", 10)), ep.online()).await {
            Ok(()) => ev("online", json!({"ms": t.elapsed().as_secs_f64() * 1000.0})),
            Err(_) => ev("online_timeout", json!({})),
        }
    }
    let stun_servers: Vec<SocketAddr> = a.all("stun").iter().filter_map(|s| s.parse().ok()).collect();
    if !stun_servers.is_empty() {
        match stun::probe(&stun_servers, 0).await {
            Ok(v) => {
                ev("stun_side", v.clone());
                if a.flag("stun-predict") {
                    if let (Some(ip), Some(port)) = (
                        v["public_ip"].as_str().and_then(|s| s.parse::<std::net::IpAddr>().ok()),
                        ep.bound_sockets().iter().find(|s| s.is_ipv4()).map(|s| s.port()),
                    ) {
                        let pred = SocketAddr::new(ip, port);
                        ep.add_external_addr(pred).await;
                        ev("stun_predicted", json!({"addr": pred.to_string()}));
                    }
                }
            }
            Err(e) => ev("stun_side_fail", json!({"err": format!("{e:#}")})),
        }
    }
    tokio::time::sleep(Duration::from_millis(a.or("settle", 3000))).await;
    let my = ep.addr();
    ev("published", addr_json(&my));
    if let Some(out) = a.get("out") {
        write_atomic(out, &addr_json(&my).to_string())?;
    }
    let lifetime = Duration::from_secs(a.or("lifetime", 40));
    let start = Instant::now();
    if a.flag("dial") {
        let pf: String = a.req("peer-file")?;
        let peer = parse_addr(&serde_json::from_str(&wait_file(&pf, 60).await?)?)?;
        let peer = filter_peer(peer, a);
        if let Some(go) = a.get("go-file") {
            let at: u128 = wait_file(go, 60).await?.trim().parse()?;
            let now = unix_ms();
            if at > now {
                tokio::time::sleep(Duration::from_millis((at - now) as u64)).await;
            }
        }
        let summary = dial(&ep, peer, a).await;
        ev("summary", summary);
    }
    let el = start.elapsed();
    if el < lifetime {
        tokio::time::sleep(lifetime - el).await;
    }
    ev("closing", json!({}));
    tokio::time::timeout(Duration::from_secs(3), ep.close()).await.ok();
    Ok(())
}

/// Runtime relay management:
/// `relaymgmt [endpoint opts] --steps "insert URL;sleep 3000;publish FILE;dial FILE;ping;remove URL;..."`
pub async fn relaymgmt(a: &Args) -> anyhow::Result<()> {
    let ep = build_endpoint(a).await?;
    spawn_watchers(&ep);
    spawn_accept(ep.clone());
    let qp: u16 = a.or("relay-quic-port", 7842);
    let mut conn: Option<Connection> = None;
    let steps: String = a.req("steps")?;
    for step in steps.split(';').map(str::trim).filter(|s| !s.is_empty()) {
        let (op, arg) = step.split_once(' ').unwrap_or((step, ""));
        let t = Instant::now();
        match op {
            "insert" | "remove" => {
                let u: RelayUrl = arg.parse()?;
                let prev = if op == "insert" {
                    ep.insert_relay(u.clone(), relay_cfg(&u, qp)).await.is_some()
                } else {
                    ep.remove_relay(&u).await.is_some()
                };
                // wait until the advertised address reflects the change (max 10 s)
                let want = op == "insert";
                let mut reflected = None;
                while t.elapsed() < Duration::from_secs(10) {
                    let has = ep.addr().addrs.iter().any(|x| matches!(x, TransportAddr::Relay(r) if *r == u));
                    if has == want {
                        reflected = Some(t.elapsed().as_secs_f64() * 1000.0);
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
                ev("step", json!({"op": op, "url": arg, "prev_existed": prev, "reflected_in_addr_ms": reflected,
                                  "addr": addr_json(&ep.addr())}));
            }
            "online" => {
                let r = tokio::time::timeout(Duration::from_secs(10), ep.online()).await.is_ok();
                ev("step", json!({"op": op, "ok": r, "ms": t.elapsed().as_secs_f64()*1000.0}));
            }
            "sleep" => tokio::time::sleep(Duration::from_millis(arg.parse()?)).await,
            "publish" => {
                write_atomic(arg, &addr_json(&ep.addr()).to_string())?;
                ev("step", json!({"op": op, "addr": addr_json(&ep.addr())}));
            }
            "dial" => {
                let peer = parse_addr(&serde_json::from_str(&wait_file(arg, 60).await?)?)?;
                let peer = filter_peer(peer, a);
                let r = tokio::time::timeout(Duration::from_secs(15), ep.connect(peer, ALPN)).await;
                match r {
                    Ok(Ok(c)) => {
                        ev("step", json!({"op": op, "ok": true, "ms": t.elapsed().as_secs_f64()*1000.0, "paths": paths_json(&c.paths())}));
                        spawn_path_logger(c.clone(), "paths", Instant::now(), Default::default());
                        conn = Some(c);
                    }
                    Ok(Err(e)) => ev("step", json!({"op": op, "ok": false, "err": format!("{e:#}")})),
                    Err(_) => ev("step", json!({"op": op, "ok": false, "err": "timeout"})),
                }
            }
            "ping" => {
                let ok = match &conn {
                    Some(c) => tokio::time::timeout(Duration::from_secs(5), echo1(c)).await.map(|r| r.is_ok()).unwrap_or(false),
                    None => false,
                };
                let sel = conn.as_ref().and_then(|c| c.paths().iter().find(|p| p.is_selected()).map(|p| format!("{:?}", p.remote_addr())));
                ev("step", json!({"op": op, "ok": ok, "ms": t.elapsed().as_secs_f64()*1000.0, "selected": sel}));
            }
            other => anyhow::bail!("unknown step {other}"),
        }
    }
    tokio::time::timeout(Duration::from_secs(3), ep.close()).await.ok();
    Ok(())
}
