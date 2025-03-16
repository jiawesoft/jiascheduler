use anyhow::Result;
use local_ip_address::local_ip;
use std::net::IpAddr;
use std::sync::{Mutex, OnceLock};

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
static COMET_ADDR: Mutex<Option<String>> = Mutex::new(None);

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
    COMET_ADDR.lock().unwrap().replace(addr.into());
}

pub fn get_comet_addr() -> Option<String> {
    COMET_ADDR.lock().unwrap().clone()
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

/// convert DateTime<Local> to local time(String)
#[macro_export]
macro_rules! local_time {
    ($time:expr) => {
        $time
            .with_timezone(&chrono::Local)
            .naive_local()
            .to_string()
    };
}

#[macro_export]
macro_rules! run_id {
    () => {
        chrono::Local::now().format("%Y%m%d%H%M%S").to_string()
    };
}
