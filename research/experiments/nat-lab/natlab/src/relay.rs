//! Embedded iroh-relay 1.2 server, as a helper desktop would run it inside the app.
//!
//! --ip <bind ip>  --cert <pem chain> --key <pem key>
//! [--https-port 443] [--http-port 80] [--quic-port 7842] [--no-qad] [--token SECRET]
use crate::{args::Args, util::*};
use iroh_relay::server::{self as rs, CertConfig, QuicConfig, RelayConfig, ServerConfig, TlsConfig};
use serde_json::json;
use std::net::{IpAddr, SocketAddr};

pub async fn run(a: &Args) -> anyhow::Result<()> {
    let ip: IpAddr = a.req("ip")?;
    let https_port: u16 = a.or("https-port", 443);
    let http_port: u16 = a.or("http-port", 80);
    let quic_port: u16 = a.or("quic-port", 7842);
    let certs = load_certs(&a.req::<String>("cert")?)?;
    let key = load_key(&a.req::<String>("key")?)?;
    let server_tls = rustls::ServerConfig::builder_with_provider(std::sync::Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()?
    .with_no_client_auth()
    .with_single_cert(certs, key)?;

    let mut relay = RelayConfig::new(SocketAddr::new(ip, http_port));
    if let Some(tok) = a.get("token") {
        // group-scoped admission: only clients presenting this token may use the relay
        relay.access = std::sync::Arc::new(TokenGate(tok.to_string()));
    }
    relay.tls = Some(TlsConfig::new(
        SocketAddr::new(ip, https_port),
        CertConfig::Manual { server_config: server_tls },
    ));
    let mut cfg = ServerConfig::default();
    cfg.relay = Some(relay);
    if !a.flag("no-qad") {
        cfg.quic = Some(QuicConfig::new(SocketAddr::new(ip, quic_port)));
    }
    let server = rs::Server::spawn(cfg).await?;
    ev(
        "relay_ready",
        json!({"http": server.http_addr().map(|x| x.to_string()),
               "https": server.https_addr().map(|x| x.to_string()),
               "quic": server.quic_addr().map(|x| x.to_string()),
               "pid": std::process::id()}),
    );
    tokio::signal::ctrl_c().await?;
    Ok(())
}

/// Admits only clients whose relay auth token equals the configured secret.
#[derive(Debug)]
struct TokenGate(String);

impl rs::AccessControl for TokenGate {
    async fn on_connect(&self, req: &rs::ClientRequest) -> rs::Access {
        let ok = req.auth_token().as_deref() == Some(self.0.as_str());
        ev("relay_access", json!({"endpoint": req.endpoint_id().to_string(), "allowed": ok}));
        if ok { rs::Access::Allow } else { rs::Access::Deny { reason: Some("bad group token".into()) } }
    }
}
