//! Mozilla Thunderbird autoconfiguration discovery.

#[cfg(feature = "client")]
pub mod client;
pub mod cname;
pub mod isp;
pub mod mailconf;
pub mod mx;

pub mod config;
