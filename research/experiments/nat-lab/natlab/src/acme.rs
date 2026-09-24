//! IP-address certificates for a helper relay.
//!
//! `acme`: instant-acme against a local Pebble, `Identifier::Ip`, `shortlived` profile, http-01
//! served by a throwaway HTTP responder on <ip>:<http-port> (port 80 only during validation).
//!   --dir https://<pebble>:14000/dir --root <pebble API CA pem> --ip <ip> [--http-port 80]
//!   [--profile shortlived] --out-cert <pem> --out-key <pem>
//! `selfsign`: CA + IP-SAN leaf (fallback that the task allows).
//!   --ip <ip> [--ip <ip2>] --out-dir <dir>
use crate::{args::Args, util::*};
use instant_acme::{
    Account, AuthorizationStatus, ChallengeType, Identifier, NewAccount, NewOrder, OrderStatus,
    RetryPolicy,
};
use rcgen::{CertificateParams, DistinguishedName, KeyPair, SanType};
use serde_json::json;
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub async fn run(a: &Args) -> anyhow::Result<()> {
    let dir: String = a.req("dir")?;
    let root: String = a.req("root")?;
    let ips: Vec<IpAddr> = a.all("ip").iter().map(|s| s.parse()).collect::<Result<_, _>>()?;
    let http_port: u16 = a.or("http-port", 80);
    let profile = a.get("profile").unwrap_or("shortlived").to_string();
    let t_start = ms();

    // http-01 responder: token -> key authorization
    let tokens: Arc<Mutex<HashMap<String, String>>> = Default::default();
    let mut listeners = Vec::new();
    for ip in &ips {
        let l = tokio::net::TcpListener::bind(SocketAddr::new(*ip, http_port)).await?;
        let tokens = tokens.clone();
        listeners.push(tokio::spawn(async move {
            loop {
                let Ok((mut s, peer)) = l.accept().await else { break };
                let tokens = tokens.clone();
                tokio::spawn(async move {
                    let mut buf = vec![0u8; 4096];
                    let n = s.read(&mut buf).await.unwrap_or(0);
                    let req = String::from_utf8_lossy(&buf[..n]).to_string();
                    let path = req.split_whitespace().nth(1).unwrap_or("").to_string();
                    let tok = path.rsplit('/').next().unwrap_or("").to_string();
                    let body = tokens.lock().unwrap().get(&tok).cloned();
                    ev("http01_request", json!({"from": peer.to_string(), "path": path, "hit": body.is_some()}));
                    let resp = match body {
                        Some(b) => format!("HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: application/octet-stream\r\nConnection: close\r\n\r\n{}", b.len(), b),
                        None => "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_string(),
                    };
                    let _ = s.write_all(resp.as_bytes()).await;
                });
            }
        }));
    }

    let (account, _creds) = Account::builder_with_root(&root)?
        .create(
            &NewAccount { contact: &[], terms_of_service_agreed: true, only_return_existing: false },
            dir.clone(),
            None,
        )
        .await?;
    let profiles: Vec<String> = account.profiles().map(|p| format!("{p:?}")).collect();
    ev("acme_account", json!({"profiles": profiles}));

    let idents: Vec<Identifier> = ips.iter().map(|ip| Identifier::Ip(*ip)).collect();
    let mut order = account.new_order(&NewOrder::new(&idents).profile(&profile)).await?;
    {
        let mut auths = order.authorizations();
        while let Some(r) = auths.next().await {
            let mut authz = r?;
            if matches!(authz.status, AuthorizationStatus::Valid) {
                continue;
            }
            let mut ch = authz
                .challenge(ChallengeType::Http01)
                .ok_or_else(|| anyhow::anyhow!("no http-01 challenge offered"))?;
            tokens.lock().unwrap().insert(ch.token.clone(), ch.key_authorization().as_str().to_string());
            ev("acme_challenge", json!({"ident": ch.identifier().to_string(), "token": ch.token}));
            ch.set_ready().await?;
        }
    }
    let retry = RetryPolicy::new().timeout(Duration::from_secs(60));
    let status = order.poll_ready(&retry).await?;
    anyhow::ensure!(status == OrderStatus::Ready, "order status {status:?}");

    let mut params = CertificateParams::new(Vec::<String>::new())?;
    params.distinguished_name = DistinguishedName::new();
    params.subject_alt_names = ips.iter().map(|ip| SanType::IpAddress(*ip)).collect();
    let key = KeyPair::generate()?;
    let csr = params.serialize_request(&key)?;
    order.finalize_csr(csr.der()).await?;
    let chain = order.poll_certificate(&retry).await?;
    std::fs::write(a.req::<String>("out-cert")?, &chain)?;
    std::fs::write(a.req::<String>("out-key")?, key.serialize_pem())?;
    ev("acme_issued", json!({"ms": ms() - t_start, "profile": profile, "chain_pems": chain.matches("BEGIN CERTIFICATE").count()}));
    for l in listeners {
        l.abort();
    }
    Ok(())
}

pub fn selfsign(a: &Args) -> anyhow::Result<()> {
    let ips: Vec<IpAddr> = a.all("ip").iter().map(|s| s.parse()).collect::<Result<_, _>>()?;
    let out: String = a.req("out-dir")?;
    std::fs::create_dir_all(&out)?;
    let ca_key = KeyPair::generate()?;
    let mut ca_p = CertificateParams::new(Vec::<String>::new())?;
    ca_p.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
    ca_p.distinguished_name.push(rcgen::DnType::CommonName, "natlab self-signed CA");
    let ca = ca_p.self_signed(&ca_key)?;
    let leaf_key = KeyPair::generate()?;
    let mut lp = CertificateParams::new(Vec::<String>::new())?;
    lp.subject_alt_names = ips.iter().map(|ip| SanType::IpAddress(*ip)).collect();
    let issuer = rcgen::Issuer::new(ca_p, ca_key);
    let leaf = lp.signed_by(&leaf_key, &issuer)?;
    std::fs::write(format!("{out}/ca.pem"), ca.pem())?;
    std::fs::write(format!("{out}/cert.pem"), leaf.pem() + &ca.pem())?;
    std::fs::write(format!("{out}/key.pem"), leaf_key.serialize_pem())?;
    ev("selfsigned", json!({"ips": ips.iter().map(|i| i.to_string()).collect::<Vec<_>>()}));
    Ok(())
}
