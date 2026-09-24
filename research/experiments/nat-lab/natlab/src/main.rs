//! natlab: iroh 1.2 NAT-lab harness for design item D49 (E10).
//!
//! Subcommands (all output is JSON lines on stdout, `t` = ms since process start):
//!   relay     embedded iroh-relay 1.2 server (HTTPS relay + QAD), manual certificate
//!   acme      issue an IP-address certificate from an ACME server (Pebble) with instant-acme
//!   selfsign  write a self-signed CA + IP-SAN leaf certificate (fallback path)
//!   stunsrv   minimal RFC 5389 STUN binding server
//!   stun      side-socket STUN probe (mapping behaviour of the NAT)
//!   peer      one iroh endpoint: publish address, accept and/or dial, observe paths
//!   relaymgmt insert_relay / remove_relay at runtime
//!   load      many relay-only endpoints for relay RAM/CPU/throughput measurements
//!   ctpeer    iroh over a custom transport (unstable API) as the first path
//!
//! Never talks to n0 infrastructure: relays are Disabled or a custom map, no address lookup.
mod acme;
mod args;
mod ct;
mod load;
mod peer;
mod relay;
mod stun;
mod util;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let filter = std::env::var("RUST_LOG").unwrap_or_else(|_| "warn".into());
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::new(filter))
        .with_writer(std::io::stderr)
        .init();
    let a = args::Args::parse();
    match a.cmd.as_str() {
        "relay" => relay::run(&a).await,
        "acme" => acme::run(&a).await,
        "selfsign" => acme::selfsign(&a),
        "stunsrv" => stun::server(&a).await,
        "stun" => stun::probe_cmd(&a).await,
        "peer" => peer::run(&a).await,
        "relaymgmt" => peer::relaymgmt(&a).await,
        "load" => load::run(&a).await,
        "ctpeer" => ct::run(&a).await,
        other => anyhow::bail!("unknown subcommand {other:?}"),
    }
}
