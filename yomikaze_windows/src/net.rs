// connection state and the offline mode switch

use std::net::{TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

static HAS_CONNECTION: AtomicBool = AtomicBool::new(true);
static FORCED_OFFLINE: AtomicBool = AtomicBool::new(false);

pub fn offline_forced() -> bool {
    FORCED_OFFLINE.load(Ordering::Relaxed)
}

pub fn set_offline_forced(value: bool) {
    FORCED_OFFLINE.store(value, Ordering::Relaxed);
}

pub fn has_connection() -> bool {
    HAS_CONNECTION.load(Ordering::Relaxed)
}

pub fn is_online() -> bool {
    has_connection() && !offline_forced()
}

fn probe() -> bool {
    let targets = ["weebcentral.com:443", "1.1.1.1:443"];
    targets.iter().any(|target| {
        target
            .to_socket_addrs()
            .ok()
            .and_then(|mut addrs| addrs.next())
            .map(|addr| TcpStream::connect_timeout(&addr, Duration::from_secs(4)).is_ok())
            .unwrap_or(false)
    })
}

// polls the connection every few seconds and wakes the ui when it changes
pub fn start_monitor(ctx: egui::Context) {
    HAS_CONNECTION.store(probe(), Ordering::Relaxed);
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(Duration::from_secs(8));
            let online = probe();
            if online != HAS_CONNECTION.swap(online, Ordering::Relaxed) {
                ctx.request_repaint();
            }
        }
    });
}
