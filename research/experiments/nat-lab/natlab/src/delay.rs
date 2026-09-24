//! WAN latency emulation without sch_netem / NFQUEUE (the WSL2 kernel has neither).
//!
//! `wanemu --ms 20 --if p-a --if p-b ...` runs inside nl-inet as a userspace learning L2 switch
//! over AF_PACKET sockets bound to the core-side veth ends (which `lab.sh delay` removes from the
//! kernel bridge). Every frame is held for --ms before it is forwarded, so host<->host RTT through
//! the core is 2*ms and a relayed RTT (client -> helper -> client) is 4*ms.
use crate::args::Args;
use std::collections::{BinaryHeap, HashMap};
use std::cmp::Reverse;
use std::time::{Duration, Instant};

struct Port {
    fd: i32,
    ifindex: i32,
    name: String,
}

fn open_port(name: &str) -> anyhow::Result<Port> {
    unsafe {
        let proto = (libc::ETH_P_ALL as u16).to_be() as i32;
        let fd = libc::socket(libc::AF_PACKET, libc::SOCK_RAW | libc::SOCK_NONBLOCK, proto);
        anyhow::ensure!(fd >= 0, "socket: {}", std::io::Error::last_os_error());
        let cname = std::ffi::CString::new(name)?;
        let ifindex = libc::if_nametoindex(cname.as_ptr()) as i32;
        anyhow::ensure!(ifindex > 0, "no interface {name}");
        let mut sll: libc::sockaddr_ll = std::mem::zeroed();
        sll.sll_family = libc::AF_PACKET as u16;
        sll.sll_protocol = proto as u16;
        sll.sll_ifindex = ifindex;
        let r = libc::bind(fd, &sll as *const _ as *const libc::sockaddr, std::mem::size_of::<libc::sockaddr_ll>() as u32);
        anyhow::ensure!(r == 0, "bind {name}: {}", std::io::Error::last_os_error());
        let sz: i32 = 8 << 20;
        libc::setsockopt(fd, libc::SOL_SOCKET, libc::SO_RCVBUF, &sz as *const _ as *const libc::c_void, 4);
        libc::setsockopt(fd, libc::SOL_SOCKET, libc::SO_SNDBUF, &sz as *const _ as *const libc::c_void, 4);
        Ok(Port { fd, ifindex, name: name.to_string() })
    }
}

fn send(p: &Port, frame: &[u8]) {
    unsafe {
        let mut sll: libc::sockaddr_ll = std::mem::zeroed();
        sll.sll_family = libc::AF_PACKET as u16;
        sll.sll_ifindex = p.ifindex;
        sll.sll_halen = 6;
        sll.sll_addr[..6].copy_from_slice(&frame[..6]);
        libc::sendto(p.fd, frame.as_ptr() as *const libc::c_void, frame.len(), 0,
            &sll as *const _ as *const libc::sockaddr, std::mem::size_of::<libc::sockaddr_ll>() as u32);
    }
}

pub fn run(a: &Args) -> anyhow::Result<()> {
    let d = Duration::from_micros((a.or("ms", 20.0f64) * 1000.0) as u64);
    let ports: Vec<Port> = a.all("if").iter().map(|n| open_port(n)).collect::<Result<_, _>>()?;
    eprintln!("wanemu: {} ports, {:?} per hop: {:?}", ports.len(), d, ports.iter().map(|p| &p.name).collect::<Vec<_>>());
    let mut macs: HashMap<[u8; 6], usize> = HashMap::new();
    // (due, seq) -> (ingress port, frame)
    let mut heap: BinaryHeap<Reverse<(Instant, u64)>> = BinaryHeap::new();
    let mut frames: HashMap<u64, (usize, Vec<u8>)> = HashMap::new();
    let mut seq = 0u64;
    let mut pfds: Vec<libc::pollfd> = ports.iter().map(|p| libc::pollfd { fd: p.fd, events: libc::POLLIN, revents: 0 }).collect();
    let mut buf = vec![0u8; 65536];
    loop {
        let now = Instant::now();
        let timeout_ms = match heap.peek() {
            Some(Reverse((t, _))) => (t.saturating_duration_since(now).as_micros() as i32 + 999) / 1000,
            None => 100,
        };
        unsafe { libc::poll(pfds.as_mut_ptr(), pfds.len() as u64, timeout_ms.min(100)) };
        for (i, p) in ports.iter().enumerate() {
            loop {
                let mut sll: libc::sockaddr_ll = unsafe { std::mem::zeroed() };
                let mut sl = std::mem::size_of::<libc::sockaddr_ll>() as u32;
                let n = unsafe {
                    libc::recvfrom(p.fd, buf.as_mut_ptr() as *mut libc::c_void, buf.len(), 0,
                        &mut sll as *mut _ as *mut libc::sockaddr, &mut sl)
                };
                if n <= 0 {
                    break;
                }
                if sll.sll_pkttype == libc::PACKET_OUTGOING as u8 || n < 14 {
                    continue;
                }
                let f = buf[..n as usize].to_vec();
                let mut src = [0u8; 6];
                src.copy_from_slice(&f[6..12]);
                macs.insert(src, i);
                seq += 1;
                heap.push(Reverse((Instant::now() + d, seq)));
                frames.insert(seq, (i, f));
            }
        }
        let now = Instant::now();
        while let Some(Reverse((t, s))) = heap.peek().copied() {
            if t > now {
                break;
            }
            heap.pop();
            let (ing, f) = frames.remove(&s).unwrap();
            let mut dst = [0u8; 6];
            dst.copy_from_slice(&f[..6]);
            match macs.get(&dst) {
                Some(&o) if o != ing => send(&ports[o], &f),
                Some(_) => {}
                None => {
                    for (o, p) in ports.iter().enumerate() {
                        if o != ing {
                            send(p, &f);
                        }
                    }
                }
            }
        }
    }
}
