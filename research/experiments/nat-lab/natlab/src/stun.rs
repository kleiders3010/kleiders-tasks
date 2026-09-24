//! Minimal RFC 5389 STUN (binding request/response, XOR-MAPPED-ADDRESS only).
use crate::{args::Args, util::*};
use serde_json::json;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::time::Duration;
use tokio::net::UdpSocket;

const COOKIE: u32 = 0x2112A442;

pub fn request(tid: [u8; 12]) -> Vec<u8> {
    let mut m = vec![0x00, 0x01, 0x00, 0x00];
    m.extend_from_slice(&COOKIE.to_be_bytes());
    m.extend_from_slice(&tid);
    m
}

fn response(tid: &[u8], from: SocketAddr) -> Vec<u8> {
    let mut attr = vec![0x00, 0x20];
    let mut val = vec![0x00];
    let xport = from.port() ^ (COOKIE >> 16) as u16;
    match from.ip() {
        IpAddr::V4(ip) => {
            val.push(0x01);
            val.extend_from_slice(&xport.to_be_bytes());
            val.extend_from_slice(&(u32::from(ip) ^ COOKIE).to_be_bytes());
        }
        IpAddr::V6(ip) => {
            val.push(0x02);
            val.extend_from_slice(&xport.to_be_bytes());
            let mut key = COOKIE.to_be_bytes().to_vec();
            key.extend_from_slice(tid);
            for (i, b) in ip.octets().iter().enumerate() {
                val.push(b ^ key[i]);
            }
        }
    }
    attr.extend_from_slice(&(val.len() as u16).to_be_bytes());
    attr.extend_from_slice(&val);
    let mut m = vec![0x01, 0x01];
    m.extend_from_slice(&(attr.len() as u16).to_be_bytes());
    m.extend_from_slice(&COOKIE.to_be_bytes());
    m.extend_from_slice(tid);
    m.extend_from_slice(&attr);
    m
}

pub fn parse_response(buf: &[u8]) -> Option<SocketAddr> {
    if buf.len() < 20 || buf[0] != 0x01 || buf[1] != 0x01 {
        return None;
    }
    let tid = &buf[8..20];
    let mut i = 20;
    while i + 4 <= buf.len() {
        let t = u16::from_be_bytes([buf[i], buf[i + 1]]);
        let l = u16::from_be_bytes([buf[i + 2], buf[i + 3]]) as usize;
        let v = buf.get(i + 4..i + 4 + l)?;
        if t == 0x0020 {
            let port = u16::from_be_bytes([v[2], v[3]]) ^ (COOKIE >> 16) as u16;
            return Some(match v[1] {
                0x01 => {
                    let x = u32::from_be_bytes([v[4], v[5], v[6], v[7]]) ^ COOKIE;
                    SocketAddr::new(IpAddr::V4(Ipv4Addr::from(x)), port)
                }
                _ => {
                    let mut key = COOKIE.to_be_bytes().to_vec();
                    key.extend_from_slice(tid);
                    let mut o = [0u8; 16];
                    for k in 0..16 {
                        o[k] = v[4 + k] ^ key[k];
                    }
                    SocketAddr::new(IpAddr::V6(Ipv6Addr::from(o)), port)
                }
            });
        }
        i += 4 + ((l + 3) & !3);
    }
    None
}

/// Binding request from an existing socket; returns the reflexive address.
pub async fn query(sock: &UdpSocket, server: SocketAddr) -> anyhow::Result<SocketAddr> {
    let tid: [u8; 12] = std::array::from_fn(|i| (ms() as u64).wrapping_add(i as u64 * 7919) as u8);
    let req = request(tid);
    let mut buf = [0u8; 512];
    for _ in 0..5 {
        sock.send_to(&req, server).await?;
        if let Ok(Ok((n, _))) = tokio::time::timeout(Duration::from_millis(500), sock.recv_from(&mut buf)).await {
            if let Some(a) = parse_response(&buf[..n]) {
                return Ok(a);
            }
        }
    }
    anyhow::bail!("no STUN answer from {server}")
}

/// `stunsrv --listen ip:port [--listen ip:port]`
pub async fn server(a: &Args) -> anyhow::Result<()> {
    let mut tasks = Vec::new();
    for l in a.all("listen") {
        let addr: SocketAddr = l.parse()?;
        let sock = UdpSocket::bind(addr).await?;
        ev("stun_listen", json!({"addr": addr.to_string()}));
        tasks.push(tokio::spawn(async move {
            let mut buf = [0u8; 1500];
            loop {
                let Ok((n, from)) = sock.recv_from(&mut buf).await else { break };
                if n >= 20 && buf[0] == 0 && buf[1] == 1 {
                    let _ = sock.send_to(&response(&buf[8..20], from), from).await;
                }
            }
        }));
    }
    for t in tasks {
        let _ = t.await;
    }
    Ok(())
}

/// Side-socket probe: `stun --server ip:port --server ip2:port [--bind-port p]`.
/// Reports local port, reflexive addresses per server, port preservation and whether
/// the mapping varies by destination (symmetric NAT).
pub async fn probe(servers: &[SocketAddr], bind_port: u16) -> anyhow::Result<serde_json::Value> {
    let sock = UdpSocket::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), bind_port)).await?;
    let local = sock.local_addr()?;
    let mut mapped = Vec::new();
    for s in servers {
        mapped.push(query(&sock, *s).await.ok());
    }
    let m: Vec<SocketAddr> = mapped.iter().flatten().copied().collect();
    let varies = m.windows(2).any(|w| w[0] != w[1]);
    let preserved = m.first().map(|x| x.port() == local.port());
    Ok(json!({
        "local_port": local.port(),
        "mapped": mapped.iter().map(|x| x.map(|y| y.to_string())).collect::<Vec<_>>(),
        "port_preserved": preserved,
        "varies_by_dest": varies,
        "public_ip": m.first().map(|x| x.ip().to_string()),
    }))
}

pub async fn probe_cmd(a: &Args) -> anyhow::Result<()> {
    let servers: Vec<SocketAddr> = a.all("server").iter().map(|s| s.parse()).collect::<Result<_, _>>()?;
    let v = probe(&servers, a.or("bind-port", 0)).await?;
    ev("stun_probe", v);
    Ok(())
}
