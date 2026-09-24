//! E3: iroh connectivity. (a) Fully local: no relays, no discovery, direct
//! addresses only. (b) Normal n0 preset: dial by endpoint ID only, which needs
//! n0's DNS/pkarr discovery. One tiny connection each.
use iroh::{Endpoint, EndpointAddr, RelayMode, endpoint::presets};
use std::time::Instant;

const ALPN: &[u8] = b"kleiders/e3/0";

async fn server(ep: Endpoint) {
    while let Some(inc) = ep.accept().await {
        tokio::spawn(async move {
            if let Ok(conn) = inc.await {
                if let Ok((mut s, mut r)) = conn.accept_bi().await {
                    let _ = tokio::io::copy(&mut r, &mut s).await;
                    let _ = s.finish();
                    conn.closed().await;
                }
            }
        });
    }
}

async fn roundtrip(client: &Endpoint, addr: EndpointAddr, bytes: usize) -> anyhow::Result<(f64, f64)> {
    let t = Instant::now();
    let conn = client.connect(addr, ALPN).await?;
    let connect_ms = t.elapsed().as_secs_f64() * 1000.0;
    let (mut s, mut r) = conn.open_bi().await?;
    let payload = vec![7u8; bytes];
    let t2 = Instant::now();
    s.write_all(&payload).await?;
    s.finish()?;
    let back = r.read_to_end(bytes + 16).await?;
    anyhow::ensure!(back.len() == bytes);
    let echo_ms = t2.elapsed().as_secs_f64() * 1000.0;
    conn.close(0u32.into(), b"bye");
    Ok((connect_ms, echo_ms))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("## (a) local only: relays disabled, no discovery");
    let a = Endpoint::builder(presets::Minimal).relay_mode(RelayMode::Disabled).alpns(vec![ALPN.to_vec()]).bind().await?;
    tokio::spawn(server(a.clone()));
    let b = Endpoint::builder(presets::Minimal).relay_mode(RelayMode::Disabled).bind().await?;
    let addr = a.addr();
    println!("server addr: {} direct addrs", addr.addrs.len());
    let (c, e) = roundtrip(&b, addr, 1_000_000).await?;
    println!("connect {c:.1} ms, 1 MB echo {e:.1} ms");

    println!("\n## (b) n0 preset: dial by endpoint ID only (needs n0 DNS/pkarr + relays)");
    let a2 = Endpoint::builder(presets::N0).alpns(vec![ALPN.to_vec()]).bind().await?;
    tokio::spawn(server(a2.clone()));
    let t = Instant::now();
    a2.online().await;
    println!("server online (home relay chosen) after {:.0} ms", t.elapsed().as_secs_f64() * 1000.0);
    let b2 = Endpoint::bind(presets::N0).await?;
    // Give the server's pkarr record a moment to publish.
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    let id_only = EndpointAddr::from(a2.id());
    match roundtrip(&b2, id_only, 10_000).await {
        Ok((c, e)) => println!("dial-by-ID: connect {c:.0} ms, 10 KB echo {e:.0} ms"),
        Err(err) => println!("dial-by-ID failed: {err:#}"),
    }
    a.close().await; b.close().await; a2.close().await; b2.close().await;
    Ok(())
}
