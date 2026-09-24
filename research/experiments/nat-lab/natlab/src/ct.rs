//! Option (c), reduced form: iroh over a custom transport (unstable API, feature
//! `unstable-custom-transports`) as the FIRST path, then watch whether iroh opens direct IP paths
//! in-band by itself.
//!
//! The custom transport is a deliberately dumb TCP datagram forwarder ("blind relay", like a
//! mini TURN without allocation): `ctrelay --listen ip:port` on a helper. Each client connects,
//! sends its 32-byte endpoint id, then exchanges frames [32-byte peer id][u16 len][payload].
//! It stands in for any non-iroh first path (libp2p circuit, HyperDHT relay, Veilid route...).
//!
//! Client side: `peer --ct-relay ip:port ...` (see peer.rs) adds this transport to the endpoint.
use crate::{args::Args, util::*};
use iroh::{
    EndpointId,
    endpoint::transports::{CustomEndpoint, CustomSender, CustomTransport, RecvInfo, Transmit},
};
use iroh_base::CustomAddr;
use serde_json::json;
use std::collections::HashMap;
use std::io;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::mpsc;

pub const CT_ID: u64 = 0x4b4c; // "KL"

pub fn to_addr(id: &EndpointId) -> CustomAddr {
    CustomAddr::from_parts(CT_ID, id.as_bytes())
}

fn from_addr(a: &CustomAddr) -> Option<[u8; 32]> {
    (a.id() == CT_ID).then(|| a.data().try_into().ok()).flatten()
}

#[derive(Debug)]
pub struct TcpRelayTransport {
    me: EndpointId,
    local: n0_watcher::Watchable<Vec<CustomAddr>>,
    out_tx: mpsc::Sender<([u8; 32], Vec<u8>)>,
    in_rx: Arc<Mutex<Option<mpsc::Receiver<([u8; 32], Vec<u8>)>>>>,
}

impl TcpRelayTransport {
    pub async fn connect(relay: SocketAddr, me: EndpointId) -> anyhow::Result<Arc<Self>> {
        let s = TcpStream::connect(relay).await?;
        s.set_nodelay(true)?;
        let (mut rd, mut wr) = s.into_split();
        wr.write_all(me.as_bytes()).await?;
        let (out_tx, mut out_rx) = mpsc::channel::<([u8; 32], Vec<u8>)>(1024);
        let (in_tx, in_rx) = mpsc::channel::<([u8; 32], Vec<u8>)>(1024);
        tokio::spawn(async move {
            while let Some((dst, p)) = out_rx.recv().await {
                let mut f = Vec::with_capacity(34 + p.len());
                f.extend_from_slice(&dst);
                f.extend_from_slice(&(p.len() as u16).to_be_bytes());
                f.extend_from_slice(&p);
                if wr.write_all(&f).await.is_err() {
                    break;
                }
            }
        });
        tokio::spawn(async move {
            let mut hdr = [0u8; 34];
            loop {
                if rd.read_exact(&mut hdr).await.is_err() {
                    break;
                }
                let len = u16::from_be_bytes([hdr[32], hdr[33]]) as usize;
                let mut p = vec![0u8; len];
                if rd.read_exact(&mut p).await.is_err() {
                    break;
                }
                let mut src = [0u8; 32];
                src.copy_from_slice(&hdr[..32]);
                let _ = in_tx.try_send((src, p)); // datagram semantics: drop when full
            }
        });
        Ok(Arc::new(Self {
            me,
            local: n0_watcher::Watchable::new(vec![to_addr(&me)]),
            out_tx,
            in_rx: Arc::new(Mutex::new(Some(in_rx))),
        }))
    }
}

#[derive(Debug)]
struct Ep {
    me: EndpointId,
    local: n0_watcher::Watchable<Vec<CustomAddr>>,
    out_tx: mpsc::Sender<([u8; 32], Vec<u8>)>,
    in_rx: mpsc::Receiver<([u8; 32], Vec<u8>)>,
}

#[derive(Debug)]
struct Sender {
    out_tx: mpsc::Sender<([u8; 32], Vec<u8>)>,
}

impl CustomTransport for TcpRelayTransport {
    fn bind(&self) -> io::Result<Box<dyn CustomEndpoint>> {
        let in_rx = self.in_rx.lock().unwrap().take().ok_or_else(|| io::Error::other("already bound"))?;
        Ok(Box::new(Ep { me: self.me, local: self.local.clone(), out_tx: self.out_tx.clone(), in_rx }))
    }
}

impl CustomSender for Sender {
    fn is_valid_send_addr(&self, addr: &CustomAddr) -> bool {
        addr.id() == CT_ID
    }
    fn poll_send(&self, _cx: &mut Context, dst: &CustomAddr, _src: Option<&CustomAddr>, t: &Transmit<'_>) -> Poll<io::Result<()>> {
        let Some(d) = from_addr(dst) else { return Poll::Ready(Err(io::Error::other("bad addr"))) };
        let seg = t.segment_size.unwrap_or(t.contents.len());
        for chunk in t.contents.chunks(seg) {
            let _ = self.out_tx.try_send((d, chunk.to_vec()));
        }
        Poll::Ready(Ok(()))
    }
}

impl CustomEndpoint for Ep {
    fn watch_local_addrs(&self) -> n0_watcher::Direct<Vec<CustomAddr>> {
        self.local.watch()
    }
    fn create_sender(&self) -> Arc<dyn CustomSender> {
        Arc::new(Sender { out_tx: self.out_tx.clone() })
    }
    fn poll_recv(&mut self, cx: &mut Context, bufs: &mut [io::IoSliceMut<'_>], metas: &mut [noq_udp::RecvMeta],
                 recv_infos: &mut [RecvInfo]) -> Poll<io::Result<usize>> {
        let n = bufs.len();
        if n == 0 {
            return Poll::Ready(Ok(0));
        }
        let mut pk = Vec::new();
        match self.in_rx.poll_recv_many(cx, &mut pk, n) {
            Poll::Pending => return Poll::Pending,
            Poll::Ready(0) => return Poll::Ready(Err(io::Error::other("ct closed"))),
            Poll::Ready(_) => {}
        }
        let mut count = 0;
        for (i, (src, data)) in pk.into_iter().enumerate() {
            if bufs[i].len() < data.len() {
                break;
            }
            bufs[i][..data.len()].copy_from_slice(&data);
            let from = CustomAddr::from_parts(CT_ID, &src);
            recv_infos[i] = RecvInfo::new(from, Some(to_addr(&self.me)));
            metas[i].len = data.len();
            metas[i].stride = data.len();
            count += 1;
        }
        if count > 0 { Poll::Ready(Ok(count)) } else { Poll::Pending }
    }
}

/// `ctrelay --listen ip:port`: blind TCP datagram forwarder keyed by endpoint id.
pub async fn run(a: &Args) -> anyhow::Result<()> {
    let listen: SocketAddr = a.req("listen")?;
    let l = tokio::net::TcpListener::bind(listen).await?;
    type Tx = mpsc::Sender<Vec<u8>>;
    let peers: Arc<Mutex<HashMap<[u8; 32], Tx>>> = Default::default();
    ev("ctrelay_ready", json!({"listen": listen.to_string()}));
    loop {
        let (s, from) = l.accept().await?;
        let peers = peers.clone();
        tokio::spawn(async move {
            let _ = s.set_nodelay(true);
            let (mut rd, mut wr) = s.into_split();
            let mut id = [0u8; 32];
            if rd.read_exact(&mut id).await.is_err() {
                return;
            }
            let (tx, mut rx) = mpsc::channel::<Vec<u8>>(1024);
            peers.lock().unwrap().insert(id, tx);
            ev("ctrelay_client", json!({"from": from.to_string()}));
            tokio::spawn(async move {
                while let Some(f) = rx.recv().await {
                    if wr.write_all(&f).await.is_err() {
                        break;
                    }
                }
            });
            let mut hdr = [0u8; 34];
            loop {
                if rd.read_exact(&mut hdr).await.is_err() {
                    break;
                }
                let len = u16::from_be_bytes([hdr[32], hdr[33]]) as usize;
                let mut p = vec![0u8; 34 + len];
                if rd.read_exact(&mut p[34..]).await.is_err() {
                    break;
                }
                let mut dst = [0u8; 32];
                dst.copy_from_slice(&hdr[..32]);
                p[..32].copy_from_slice(&id);
                p[32..34].copy_from_slice(&hdr[32..34]);
                let tx = peers.lock().unwrap().get(&dst).cloned();
                if let Some(tx) = tx {
                    let _ = tx.try_send(p);
                }
            }
            peers.lock().unwrap().remove(&id);
        });
    }
}
