//! Relay load: N relay-only endpoints (half dial the other half through the helper relay).
//! `load --relay URL --ca PEM --n 100 --relay-pid PID [--hold 20] [--ping-ms 1000]
//!       [--tp-pairs 1,5,10] [--tp-bytes 20000000]`
use crate::{args::Args, peer, util::*};
use iroh::{
    Endpoint, EndpointAddr, RelayMap, RelayMode, RelayUrl, TransportAddr,
    endpoint::{Connection, PortmapperConfig, presets},
};
use iroh_relay::tls::CaTlsConfig;
use serde_json::json;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

fn relay_stats(pid: u32) -> serde_json::Value {
    match proc_stats(pid) {
        Some((rss, ticks)) => json!({"rss_kb": rss, "cpu_ticks": ticks}),
        None => json!(null),
    }
}

async fn cpu_during<F: std::future::Future>(pid: u32, f: F) -> (F::Output, Option<f64>, u64) {
    let hz = 100.0; // USER_HZ on Linux
    let before = proc_stats(pid);
    let t = Instant::now();
    // sample peak RSS while running
    let peak = Arc::new(AtomicU64::new(0));
    let p2 = peak.clone();
    let sampler = tokio::spawn(async move {
        loop {
            if let Some((rss, _)) = proc_stats(pid) {
                p2.fetch_max(rss, Ordering::Relaxed);
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    });
    let out = f.await;
    sampler.abort();
    let after = proc_stats(pid);
    let cpu = match (before, after) {
        (Some((_, b)), Some((_, a))) => Some((a - b) as f64 / hz / t.elapsed().as_secs_f64() * 100.0),
        _ => None,
    };
    (out, cpu, peak.load(Ordering::Relaxed))
}

async fn echo(conn: &Connection) -> bool {
    async {
        let (mut s, mut r) = conn.open_bi().await?;
        s.write_all(&[1]).await?;
        s.finish()?;
        r.read_to_end(8).await?;
        anyhow::Ok(())
    }
    .await
    .is_ok()
}

async fn push(conn: Connection, bytes: usize) -> anyhow::Result<usize> {
    let (mut s, mut r) = conn.open_bi().await?;
    let chunk = vec![3u8; 64 * 1024];
    let mut left = bytes;
    while left > 0 {
        let n = left.min(chunk.len());
        s.write_all(&chunk[..n]).await?;
        left -= n;
    }
    s.finish()?;
    r.read_to_end(16).await?;
    Ok(bytes)
}

pub async fn run(a: &Args) -> anyhow::Result<()> {
    let n: usize = a.or("n", 100);
    let url: RelayUrl = a.req::<String>("relay")?.parse()?;
    let pid: u32 = a.req("relay-pid")?;
    let roots = load_certs(&a.req::<String>("ca")?)?;
    ev("relay_baseline", relay_stats(pid));

    let t = Instant::now();
    let mut eps: Vec<Endpoint> = Vec::new();
    for _ in 0..n {
        let ep = Endpoint::builder(presets::Minimal)
            .relay_mode(RelayMode::Custom(RelayMap::from_iter([peer::relay_cfg(&url, 7842)])))
            .ca_tls_config(CaTlsConfig::custom_roots(roots.clone()))
            .portmapper_config(PortmapperConfig::Disabled)
            .clear_ip_transports()
            .alpns(vec![peer::ALPN.to_vec()])
            .bind()
            .await?;
        peer::spawn_accept(ep.clone());
        eps.push(ep);
    }
    let online = futures::future::join_all(
        eps.iter().map(|e| tokio::time::timeout(Duration::from_secs(30), e.online())),
    )
    .await
    .iter()
    .filter(|r| r.is_ok())
    .count();
    ev("load_online", json!({"n": n, "online": online, "ms": t.elapsed().as_millis(), "relay": relay_stats(pid)}));

    let half = n / 2;
    let t = Instant::now();
    let conns: Vec<Option<Connection>> = futures::future::join_all((0..half).map(|i| {
        let ep = eps[i].clone();
        let target = EndpointAddr::from_parts(eps[i + half].id(), [TransportAddr::Relay(url.clone())]);
        async move {
            tokio::time::timeout(Duration::from_secs(30), ep.connect(target, peer::ALPN))
                .await
                .ok()
                .and_then(|r| r.ok())
        }
    }))
    .await;
    let conns: Vec<Connection> = conns.into_iter().flatten().collect();
    ev("load_connected", json!({"pairs": half, "connected": conns.len(), "ms": t.elapsed().as_millis(),
                                 "relay": relay_stats(pid)}));

    // Hold phase: every connection pings once per interval.
    let hold = Duration::from_secs(a.or("hold", 20));
    let ping = Duration::from_millis(a.or("ping-ms", 1000));
    let ok = Arc::new(AtomicU64::new(0));
    let fail = Arc::new(AtomicU64::new(0));
    let (_, cpu, peak) = cpu_during(pid, async {
        let start = Instant::now();
        while start.elapsed() < hold {
            let rs = futures::future::join_all(conns.iter().map(echo)).await;
            for r in rs {
                if r { ok.fetch_add(1, Ordering::Relaxed); } else { fail.fetch_add(1, Ordering::Relaxed); }
            }
            tokio::time::sleep(ping).await;
        }
    })
    .await;
    ev("load_hold", json!({"secs": hold.as_secs(), "echo_ok": ok.load(Ordering::Relaxed),
        "echo_fail": fail.load(Ordering::Relaxed), "relay_cpu_pct": cpu, "relay_peak_rss_kb": peak,
        "relay": relay_stats(pid)}));

    // Throughput through the relay with k concurrent pairs.
    let bytes: usize = a.or("tp-bytes", 20_000_000);
    for k in a.get("tp-pairs").unwrap_or("1,5,10").split(',').filter_map(|s| s.parse::<usize>().ok()) {
        let k = k.min(conns.len());
        let t = Instant::now();
        let (res, cpu, peak) = cpu_during(pid, futures::future::join_all(
            conns.iter().take(k).map(|c| tokio::time::timeout(Duration::from_secs(120), push(c.clone(), bytes))),
        ))
        .await;
        let done: usize = res.into_iter().filter_map(|r| r.ok().and_then(|x| x.ok())).sum();
        let secs = t.elapsed().as_secs_f64();
        ev("load_throughput", json!({"pairs": k, "bytes_each": bytes, "bytes_done": done,
            "secs": secs, "aggregate_mbps": done as f64 * 8.0 / secs / 1e6,
            "relay_cpu_pct": cpu, "relay_peak_rss_kb": peak}));
    }
    for e in &eps {
        let _ = tokio::time::timeout(Duration::from_secs(2), e.close()).await;
    }
    tokio::time::sleep(Duration::from_secs(3)).await;
    ev("load_after_close", json!({"relay": relay_stats(pid)}));
    Ok(())
}
