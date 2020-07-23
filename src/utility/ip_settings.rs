// Copyright (c) SimpleStaking and Tezedge Contributors
// SPDX-License-Identifier: MIT

use std::{
    net::IpAddr,
    process::Command,
};
use std::str::FromStr;

/// Detect local IP address
pub fn get_local_ip() -> Option<IpAddr> {
    std::env::args().nth(2)
        .map(|value| IpAddr::from_str(&value).ok())
        .flatten().or_else(get_ip_from_hostname)
}

/// Try to get local IP from the system hostname
fn get_ip_from_hostname() -> Option<IpAddr> {
    IpAddr::from_str(String::from_utf8(
        Command::new("hostname").args(&["-I"])
            .output().ok()?.stdout
    ).ok()?.trim().split_whitespace().next()?).ok()
}