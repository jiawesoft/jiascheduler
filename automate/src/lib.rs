use anyhow::Result;
use local_ip_address::local_ip;
use std::net::IpAddr;
use std::sync::OnceLock;

pub mod bridge;
pub mod comet;
pub mod scheduler;
pub mod ssh;
pub use bridge::msg::DispatchJobParams;
pub use comet::logic::Logic;
pub use comet::types::{
    DispatchJobRequest, LinkPair, SftpDownloadRequest, SftpReadDirRequest, SftpRemoveRequest,
    SftpUploadRequest,
};
use reqwest::Client;
pub use scheduler::types::BaseJob;
pub use scheduler::types::JobAction;

pub mod bus;

static LOCAL_IP: OnceLock<IpAddr> = OnceLock::new();
static HTTP_CLIENT: OnceLock<Client> = OnceLock::new();
static mut COMET_ADDR: OnceLock<String> = OnceLock::new();

pub fn get_local_ip() -> IpAddr {
    let ip = LOCAL_IP.get_or_init(|| local_ip().expect("failed get local ip"));
    ip.to_owned()
}

pub fn get_endpoint(ip: impl Into<String>, mac_address: impl Into<String>) -> String {
    let (ip, mac_address) = (ip.into(), mac_address.into());
    format!("jiascheduler:ins:{ip}:{mac_address}")
}

pub fn get_http_client() -> Client {
    let c = HTTP_CLIENT.get_or_init(|| reqwest::Client::new());
    c.clone()
}

pub fn set_comet_addr(addr: impl Into<String>) {
    unsafe {
        if let Some(v) = COMET_ADDR.get_mut() {
            *v = addr.into()
        } else {
            COMET_ADDR.set(addr.into()).expect("failed set comet addr");
        }
    }
}

pub fn get_comet_addr() -> Option<String> {
    unsafe { COMET_ADDR.get().cloned() }
}

pub fn get_mac_address() -> Result<String> {
    match mac_address::get_mac_address()? {
        Some(ma) => Ok(ma.to_string()),
        None => anyhow::bail!("No MAC address found."),
    }
}

#[test]
fn test_get_mac_address() {
    let ret = get_mac_address();
    assert_eq!(ret.is_ok(), true);
}

/// convert DateTime<Utc> to local time(String)
#[macro_export]
macro_rules! local_time {
    ($time:expr) => {
        $time
            .with_timezone(&chrono::Local)
            .naive_local()
            .to_string()
    };
}
