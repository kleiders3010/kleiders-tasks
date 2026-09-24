//! Size probe: veilid-core startup, attach, DHT record create/set, app_message.
use std::ffi::c_int;
use std::sync::Arc;
use veilid_core::*;

#[unsafe(no_mangle)]
pub extern "C" fn probe_veilid(_n: c_int) -> c_int {
    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
    rt.block_on(async {
        let cb: UpdateCallback = Arc::new(|_u| {});
        let api = api_startup_json(cb, r#"{"program_name":"kt","namespace":"probe"}"#.to_string()).await.unwrap();
        api.attach().await.unwrap();
        let rc = api.routing_context().unwrap();
        let rec = rc.create_dht_record(CRYPTO_KIND_VLD0, DHTSchema::dflt(1).unwrap(), None).await.unwrap();
        let _ = rc.set_dht_value(rec.key().clone(), 0, b"slot".to_vec(), None).await;
        api.shutdown().await;
        1
    })
}
