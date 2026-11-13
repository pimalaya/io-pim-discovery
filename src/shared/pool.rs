//! # Stream pool
//!
//! Lazily-opened, URL-keyed cache of [`DiscoveryStream`] streams for the
//! sans-I/O coroutine runtimes that talk to multiple endpoints in a
//! single discovery cycle (PACC, autoconfig, …).
//!
//! On a [`get`] miss, the pool dispatches to a factory closure
//! registered for the URL scheme. A default `tcp` factory is
//! pre-registered by [`new`]; callers plug additional schemes
//! (typically `http`/`https` via [`with_http_factories`]) through
//! [`with_factory`].
//!
//! [`new`]: DiscoveryStreamPool::new
//! [`get`]: DiscoveryStreamPool::get
//! [`with_factory`]: DiscoveryStreamPool::with_factory
//! [`with_http_factories`]: DiscoveryStreamPool::with_http_factories

use std::{
    boxed::Box,
    collections::HashMap,
    io::{Read, Write},
    net::{TcpStream, ToSocketAddrs},
    string::String,
    time::Duration,
};

use anyhow::{Error, Result, bail};
use log::trace;
use url::Url;

/// Connect, read and write timeout of the default `tcp` factory.
const TCP_TIMEOUT: Duration = Duration::from_secs(10);

/// Open marker for everything the pool can store. Auto-implemented
/// for any blocking `Read + Write`.
pub trait DiscoveryStream: Read + Write {}
impl<T: Read + Write + ?Sized> DiscoveryStream for T {}

type StreamFactory = Box<dyn FnMut(&Url) -> Result<Box<dyn DiscoveryStream>>>;

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct StreamPoolKey {
    scheme: String,
    host: String,
    port: u16,
}

impl TryFrom<&Url> for StreamPoolKey {
    type Error = Error;

    fn try_from(url: &Url) -> Result<Self> {
        let scheme = url.scheme().to_ascii_lowercase();

        let Some(host) = url.host_str() else {
            bail!("Stream pool URL `{url}` has no host");
        };

        let Some(port) = url.port_or_known_default() else {
            bail!("Stream pool URL `{url}` has no known port")
        };

        Ok(StreamPoolKey {
            scheme,
            host: host.to_ascii_lowercase(),
            port,
        })
    }
}

/// URL-keyed pool of lazily-opened blocking streams.
pub struct DiscoveryStreamPool {
    streams: HashMap<StreamPoolKey, Box<dyn DiscoveryStream>>,
    factories: HashMap<&'static str, StreamFactory>,
}

impl Default for DiscoveryStreamPool {
    fn default() -> Self {
        Self::new()
    }
}

impl DiscoveryStreamPool {
    /// Builds a pool with the default `tcp` factory pre-registered,
    /// timing out after ten seconds ([`with_tcp_factory`] sets
    /// another). Callers plug additional schemes (typically
    /// `http`/`https` via [`with_http_factories`]) through
    /// [`with_factory`].
    ///
    /// [`with_factory`]: DiscoveryStreamPool::with_factory
    /// [`with_http_factories`]: DiscoveryStreamPool::with_http_factories
    /// [`with_tcp_factory`]: DiscoveryStreamPool::with_tcp_factory
    pub fn new() -> Self {
        let pool = Self {
            streams: HashMap::new(),
            factories: HashMap::new(),
        };

        pool.with_tcp_factory(TCP_TIMEOUT)
    }

    /// Registers the `tcp` factory: plain TCP streams whose connect,
    /// reads and writes each give up after `timeout`.
    pub fn with_tcp_factory(self, timeout: Duration) -> Self {
        self.with_factory("tcp", move |url: &Url| -> Result<TcpStream> {
            let Some(host) = url.host_str() else {
                bail!("TCP URL `{url}` has no host");
            };

            let Some(port) = url.port_or_known_default() else {
                bail!("TCP URL `{url}` has no port");
            };

            let mut failure = None;

            for addr in (host, port).to_socket_addrs()? {
                match TcpStream::connect_timeout(&addr, timeout) {
                    Ok(stream) => {
                        stream.set_read_timeout(Some(timeout))?;
                        stream.set_write_timeout(Some(timeout))?;
                        return Ok(stream);
                    }
                    Err(err) => failure = Some(err),
                }
            }

            match failure {
                Some(err) => Err(err.into()),
                None => bail!("TCP URL `{url}` resolves to no address"),
            }
        })
    }

    /// Registers (or replaces) the factory for `scheme`. Pass a
    /// lowercase literal (`"https"`, `"tcp"`, …); URL lookups are
    /// lowercased before matching.
    pub fn with_factory<F, S>(mut self, scheme: &'static str, mut factory: F) -> Self
    where
        F: FnMut(&Url) -> Result<S> + 'static,
        S: DiscoveryStream + 'static,
    {
        let boxed: StreamFactory =
            Box::new(move |url| factory(url).map(|s| Box::new(s) as Box<dyn DiscoveryStream>));
        self.factories.insert(scheme, boxed);
        self
    }

    /// Registers `http` and `https` factories backed by pimalaya-stream.
    /// The plain `http` factory opens a TCP socket; the `https` factory
    /// runs the TLS handshake described by `tls`.
    ///
    /// HTTPS connections fail at runtime if no TLS provider feature is
    /// enabled on `pimalaya-stream`. Gated by the `stream` feature;
    /// light clients that do not pull `pimalaya-stream` register their
    /// own `http`/`https` factories via [`with_factory`].
    ///
    /// [`with_factory`]: Self::with_factory
    #[cfg(feature = "stream")]
    pub fn with_http_factories(self, tls: pimalaya_stream::tls::Tls) -> Self {
        use pimalaya_stream::stream::{Stream, TcpConnectOptions, TlsConnectOptions};

        self.with_factory("http", |url: &Url| -> Result<Stream> {
            let Some(host) = url.host_str() else {
                bail!("HTTP URL `{url}` has no host");
            };
            let port = url.port_or_known_default().unwrap_or(80);
            Stream::connect_tcp(host, port, TcpConnectOptions::default())
        })
        .with_factory("https", move |url: &Url| -> Result<Stream> {
            let Some(host) = url.host_str() else {
                bail!("HTTPS URL `{url}` has no host");
            };
            let port = url.port_or_known_default().unwrap_or(443);
            let opts = TlsConnectOptions {
                tls: tls.clone(),
                ..Default::default()
            };

            Stream::connect_tls(host, port, opts)
        })
    }

    /// Unregisters the factory for `scheme`, so that no stream opens
    /// for it (a secure-only pool drops `http`).
    pub fn without_factory(mut self, scheme: &str) -> Self {
        self.factories.remove(scheme);
        self
    }

    /// Pre-feeds a stream for one specific URL. Bypasses the scheme
    /// factory for that URL.
    pub fn insert(&mut self, url: &Url, stream: impl DiscoveryStream + 'static) {
        if let Ok(key) = url.try_into() {
            self.streams.insert(key, Box::new(stream));
        }
    }

    /// Returns a mutable reference to the stream open on `url`,
    /// opening one via the factory registered for `url.scheme()` if
    /// the cache misses.
    pub fn get(&mut self, url: &Url) -> Result<&mut dyn DiscoveryStream> {
        let key = url.try_into()?;

        if !self.streams.contains_key(&key) {
            trace!("opening pool stream for {url}");

            let stream = {
                let Some(factory) = self.factories.get_mut(key.scheme.as_str()) else {
                    bail!("No stream factory registered for scheme `{}`", key.scheme)
                };

                factory(url)?
            };

            self.streams.insert(key.clone(), stream);
        }

        Ok(self.streams.get_mut(&key).unwrap().as_mut())
    }
}

#[cfg(test)]
mod tests {
    use std::{
        net::{TcpListener, TcpStream},
        string::ToString,
        thread,
        time::{Duration, Instant},
    };

    use anyhow::{Result, bail};
    use url::Url;

    use crate::shared::pool::DiscoveryStreamPool;

    #[test]
    fn unregistered_scheme_opens_no_stream() {
        let refuse = |_: &Url| -> Result<TcpStream> { bail!("factory reached") };
        let mut pool = DiscoveryStreamPool::new()
            .with_factory("http", refuse)
            .without_factory("http");

        let url = Url::parse("http://example.com/").unwrap();
        let err = pool.get(&url).err().unwrap();

        assert!(err.to_string().starts_with("No stream factory"));
    }

    #[test]
    fn the_tcp_stream_gives_up_on_a_silent_peer() {
        // A peer that accepts, says nothing for two seconds, then closes.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        thread::spawn(move || {
            let peer = listener.accept();
            thread::sleep(Duration::from_secs(2));
            drop(peer);
        });

        let timeout = Duration::from_millis(200);
        let mut pool = DiscoveryStreamPool::new().with_tcp_factory(timeout);
        let url = Url::parse(&format!("tcp://127.0.0.1:{port}")).unwrap();
        let stream = pool.get(&url).unwrap();

        let started = Instant::now();
        let read = stream.read(&mut [0; 16]);

        assert!(read.is_err(), "read ended on {read:?}, not on its deadline");
        assert!(started.elapsed() < Duration::from_secs(1));
    }
}
