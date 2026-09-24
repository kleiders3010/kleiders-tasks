use std::sync::OnceLock;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

static T0: OnceLock<Instant> = OnceLock::new();

pub fn ms() -> f64 {
    T0.get_or_init(Instant::now).elapsed().as_secs_f64() * 1000.0
}

pub fn unix_ms() -> u128 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis()
}

/// Print one JSON event line.
pub fn ev(name: &str, mut v: serde_json::Value) {
    if let Some(o) = v.as_object_mut() {
        o.insert("t".into(), serde_json::json!((ms() * 10.0).round() / 10.0));
        o.insert("ev".into(), serde_json::json!(name));
    }
    println!("{v}");
}

pub fn load_certs(path: &str) -> anyhow::Result<Vec<rustls::pki_types::CertificateDer<'static>>> {
    let mut rd = std::io::BufReader::new(std::fs::File::open(path)?);
    Ok(rustls_pemfile::certs(&mut rd).collect::<Result<Vec<_>, _>>()?)
}

pub fn load_key(path: &str) -> anyhow::Result<rustls::pki_types::PrivateKeyDer<'static>> {
    let mut rd = std::io::BufReader::new(std::fs::File::open(path)?);
    rustls_pemfile::private_key(&mut rd)?.ok_or_else(|| anyhow::anyhow!("no key in {path}"))
}

/// Resident set size and CPU ticks of a process (Linux /proc).
#[allow(dead_code)]
pub fn proc_stats(pid: u32) -> Option<(u64, u64)> {
    let status = std::fs::read_to_string(format!("/proc/{pid}/status")).ok()?;
    let rss_kb = status
        .lines()
        .find(|l| l.starts_with("VmRSS:"))?
        .split_whitespace()
        .nth(1)?
        .parse()
        .ok()?;
    let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
    let rest = &stat[stat.rfind(')')? + 2..];
    let f: Vec<&str> = rest.split_whitespace().collect();
    let ticks: u64 = f.get(11)?.parse::<u64>().ok()? + f.get(12)?.parse::<u64>().ok()?;
    Some((rss_kb, ticks))
}
