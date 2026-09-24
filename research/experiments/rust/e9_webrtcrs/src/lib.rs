//! Size probe: webrtc-rs 0.21 peer connection with a data channel and an offer (no media codecs).
use std::ffi::c_int;
use webrtc::peer_connection::{PeerConnection, PeerConnectionBuilder, PeerConnectionEventHandler, RTCConfigurationBuilder, RTCIceServer};
use webrtc::runtime::default_runtime;

#[derive(Clone)]
struct H;
#[async_trait::async_trait]
impl PeerConnectionEventHandler for H {}

#[unsafe(no_mangle)]
pub extern "C" fn probe_webrtc(port: c_int) -> c_int {
    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
    rt.block_on(async move {
        let config = RTCConfigurationBuilder::new()
            .with_ice_servers(vec![RTCIceServer { urls: vec!["stun:stun.example.org:3478".to_string()], ..Default::default() }])
            .build();
        let pc = PeerConnectionBuilder::new()
            .with_configuration(config)
            .with_handler(std::sync::Arc::new(H))
            .with_runtime(default_runtime().unwrap())
            .with_udp_addrs(vec![format!("0.0.0.0:{port}")])
            .build().await.unwrap();
        let _dc = pc.create_data_channel("kt", None).await.unwrap();
        let offer = pc.create_offer(None).await.unwrap();
        let _ = pc.close().await;
        offer.sdp.len() as c_int
    })
}
