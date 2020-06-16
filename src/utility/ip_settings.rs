use std::{
    net::IpAddr,
    process::Command,
};
use std::str::FromStr;

pub fn get_local_ip() -> Option<IpAddr> {
    IpAddr::from_str(String::from_utf8(
        Command::new("hostname").args(&["-I"])
            .output().ok()?.stdout
    ).ok()?.trim().split_whitespace().next()?).ok()
}