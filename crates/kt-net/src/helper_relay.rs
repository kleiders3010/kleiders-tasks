//! Helper relay: an iroh relay server embedded in the app (DESIGN §6/§7a, D49 option (a)).
//!
//! Desktops with a public IPv4 address or a UPnP router can relay live connections for
//! their groups' devices when those can't connect directly (E10: 5 of 13 NAT pairs,
//! including phone↔phone on carrier NAT, only ever connect through a relay).
//!
//! - **Access:** open, or restricted to holders of a token (e.g. a per-group secret).
//!   Whether default-on helpers serve strangers is D51 (the user's decision, pending);
//!   the capability exists either way.
//! - **Per-client bandwidth cap** (E10: bulk relaying costs ~25 core-seconds per GB).
//! - **TLS:** real helpers need a certificate for their IP address (Let's Encrypt IP
//!   certificates; E10 validated the flow against a local ACME server). This prototype
//!   serves plain HTTP (LAN, tests, USB tunnels) or a caller-provided certificate. The ACME
//!   automation is not built yet: it needs a publicly reachable machine to test.

use std::collections::HashSet;
use std::net::SocketAddr;
use std::num::NonZeroU32;
use std::sync::Arc;

use iroh_relay::server::{self as rs, AccessControl, ClientRateLimit, Limits, RelayConfig, ServerConfig};

#[derive(Clone, Debug)]
pub struct HelperRelayConfig {
    pub http_bind: SocketAddr,
    /// `None`: anyone may relay through us. `Some`: only clients presenting one of these.
    pub tokens: Option<HashSet<String>>,
    /// Per-client receive cap in bytes per second.
    pub client_bytes_per_sec: u32,
}

impl Default for HelperRelayConfig {
    fn default() -> Self {
        HelperRelayConfig { http_bind: "127.0.0.1:0".parse().expect("addr"), tokens: None, client_bytes_per_sec: 512 * 1024 }
    }
}

#[derive(Debug)]
struct TokenGate(HashSet<String>);

impl AccessControl for TokenGate {
    async fn on_connect(&self, req: &rs::ClientRequest) -> rs::Access {
        match req.auth_token() {
            Some(t) if self.0.contains(t.as_str()) => rs::Access::Allow,
            _ => rs::Access::Deny { reason: Some("not allowed on this relay".into()) },
        }
    }
}

pub struct HelperRelay {
    server: rs::Server,
}

impl HelperRelay {
    pub async fn start(cfg: HelperRelayConfig) -> Result<HelperRelay, String> {
        let mut relay = RelayConfig::new(cfg.http_bind);
        let mut limits = Limits::default();
        let mut rl = ClientRateLimit::new(NonZeroU32::new(cfg.client_bytes_per_sec.max(1)).expect("non-zero"));
        rl.max_burst_bytes = NonZeroU32::new(cfg.client_bytes_per_sec.saturating_mul(4));
        limits.client_rx = Some(rl);
        relay.limits = limits;
        if let Some(t) = cfg.tokens {
            relay.access = Arc::new(TokenGate(t));
        }
        let mut sc = ServerConfig::default();
        sc.relay = Some(relay);
        let server = rs::Server::spawn(sc).await.map_err(|e| e.to_string())?;
        Ok(HelperRelay { server })
    }

    pub fn http_addr(&self) -> Option<SocketAddr> {
        self.server.http_addr()
    }

    /// The URL clients put in their relay map (plain HTTP in this prototype).
    pub fn url(&self) -> Option<String> {
        self.http_addr().map(|a| format!("http://{a}"))
    }

    pub async fn shutdown(self) {
        let _ = self.server.shutdown().await;
    }
}
