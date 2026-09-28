//! Plumbing shared across the discovery mechanisms: the DNS transport
//! and record coroutines, the HTTP GET helper, the redirect walk and
//! the std client's stream pool.

pub mod dns;
#[cfg(any(feature = "autoconfig", feature = "pacc"))]
pub mod http;
#[cfg(feature = "client")]
pub mod pool;
#[cfg(any(feature = "rfc6764", feature = "rfc8620"))]
pub mod walk;
