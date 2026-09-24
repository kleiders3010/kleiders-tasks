//! E6: size probe. Each feature pulls in one heavy dependency and uses it enough
//! that the linker can't drop it.
use std::ffi::c_int;

#[unsafe(no_mangle)]
pub extern "C" fn probe_crypto(n: c_int) -> c_int {
    use argon2::Argon2;
    use chacha20poly1305::{aead::{Aead, KeyInit}, XChaCha20Poly1305, XNonce};
    let mut key = [0u8; 32];
    Argon2::default().hash_password_into(b"maple-orbit", b"kleiders-salt-16", &mut key).unwrap();
    let hk = hkdf::Hkdf::<sha2::Sha256>::new(None, &key);
    let mut okm = [0u8; 32];
    hk.expand(b"group-doc-key", &mut okm).unwrap();
    let c = XChaCha20Poly1305::new((&okm).into());
    let ct = c.encrypt(XNonce::from_slice(&[0u8; 24]), vec![n as u8; 64].as_ref()).unwrap();
    ct.len() as c_int
}

#[cfg(feature = "loro")]
#[unsafe(no_mangle)]
pub extern "C" fn probe_loro(n: c_int) -> c_int {
    let doc = loro::LoroDoc::new();
    let tree = doc.get_tree("t");
    for i in 0..n { let id = tree.create(None).unwrap(); tree.get_meta(id).unwrap().insert("title", i as i64).unwrap(); }
    doc.get_text("d").insert(0, "hello").unwrap();
    doc.get_movable_list("l").push(1).unwrap();
    doc.commit();
    let snap = doc.export(loro::ExportMode::Snapshot).unwrap();
    let d2 = loro::LoroDoc::from_snapshot(&snap).unwrap();
    d2.import(&doc.export(loro::ExportMode::updates(&Default::default())).unwrap()).unwrap();
    snap.len() as c_int
}

#[cfg(feature = "nostr")]
#[unsafe(no_mangle)]
pub extern "C" fn probe_nostr(n: c_int) -> c_int {
    use nostr::prelude::*;
    let keys = Keys::generate();
    let ev = EventBuilder::new(Kind::Custom(30078), format!("x{n}")).tag(Tag::identifier("slot")).finalize(&keys).unwrap();
    ev.as_json().len() as c_int
}

#[cfg(feature = "iroh")]
#[unsafe(no_mangle)]
pub extern "C" fn probe_iroh(_n: c_int) -> c_int {
    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
    rt.block_on(async {
        let ep = iroh::Endpoint::builder(iroh::endpoint::presets::N0).alpns(vec![b"kt/1".to_vec()]).bind().await.unwrap();
        let s = ep.addr().id.to_string().len();
        if let Some(inc) = ep.accept().await { if let Ok(conn) = inc.await { let _ = conn.accept_bi().await; } }
        let c = ep.connect(ep.addr(), b"kt/1").await.ok().map(|c| c.stable_id()).unwrap_or(0);
        ep.close().await;
        (s + c) as c_int
    })
}
