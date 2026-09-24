//! Size probe: str0m (sans-IO WebRTC) with a data channel offer/answer and polling.
use std::ffi::c_int;
use std::time::Instant;
use str0m::{Candidate, Rtc, Output};

#[unsafe(no_mangle)]
pub extern "C" fn probe_str0m(port: c_int) -> c_int {
    let mut a = Rtc::new(Instant::now());
    let mut b = Rtc::new(Instant::now());
    let addr = format!("127.0.0.1:{port}").parse().unwrap();
    a.add_local_candidate(Candidate::host(addr, "udp").unwrap());
    b.add_local_candidate(Candidate::host(addr, "udp").unwrap());
    let mut change = a.sdp_api();
    let _cid = change.add_channel("kt".into());
    let (offer, pending) = change.apply().unwrap();
    let answer = b.sdp_api().accept_offer(offer).unwrap();
    a.sdp_api().accept_answer(pending, answer).unwrap();
    let mut n = 0;
    loop {
        match a.poll_output().unwrap() {
            Output::Timeout(_) => break,
            _ => n += 1,
        }
    }
    n
}
