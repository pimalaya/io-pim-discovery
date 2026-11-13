//! # Combined RFC 8620 resolve coroutine
//!
//! [`DiscoveryJmapResolve`] turns a bare domain into a JMAP Session resource
//! URL by chaining the two RFC 8620 §2.2 mechanisms:
//!
//! 1. An SRV lookup on `_jmap._tcp.<domain>`, picking the best record
//!    to fix the origin host and port (the session is always HTTPS).
//! 2. A `.well-known/jmap` probe on that origin, following any
//!    redirect chain to the session resource.
//!
//! When the domain publishes no SRV record it falls back to
//! `https://<domain>:443/` and probes `.well-known/jmap` there. An
//! origin whose redirect chain ends on neither a session object nor a
//! challenged 401 means no JMAP service: the resolve completes with
//! [`DiscoveryJmapResolveError::NotFound`].
//!
//! Composing the DNS step (over the `tcp://` resolver) and the HTTPS
//! well-known step in one coroutine works because every yield carries
//! its endpoint URL: the std client routes each step through the
//! matching stream in its [`DiscoveryStreamPool`].
//!
//! [`DiscoveryStreamPool`]: crate::shared::pool::DiscoveryStreamPool

use core::mem;

use alloc::{
    boxed::Box,
    format,
    string::{String, ToString},
};

use log::{debug, trace};
use thiserror::Error;
use url::Url;

use crate::{
    coroutine::{DiscoveryCoroutine, DiscoveryCoroutineState, DiscoveryYield},
    rfc6186::srv::DiscoveryDnsSrv,
    rfc8620::well_known::{
        DiscoveryJmapSessionResource, DiscoveryJmapWellKnown, DiscoveryJmapWellKnownError,
    },
};

/// Errors emitted by [`DiscoveryJmapResolve`].
#[derive(Debug, Error)]
pub enum DiscoveryJmapResolveError {
    /// An error occurred during the `.well-known/jmap` probe step.
    #[error(transparent)]
    DiscoveryWellKnown(#[from] DiscoveryJmapWellKnownError),
    /// The SRV record yielded a host/port that produced a URL that
    /// could not be parsed.
    #[error("RFC 8620 resolve built an invalid origin URL `{0}`: {1}")]
    InvalidOrigin(String, #[source] url::ParseError),
    /// No JMAP session resource was found at the resolved origin.
    #[error("RFC 8620 resolve found no JMAP session on `{0}`")]
    NotFound(Url),
}

/// The session resource found by a [`DiscoveryJmapResolve`].
#[derive(Clone, Debug)]
pub struct DiscoveryJmapResolveOutput {
    /// The JMAP Session resource.
    pub session: DiscoveryJmapSessionResource,
    /// Whether an SRV record named the origin, not the domain itself.
    pub srv: bool,
}

/// I/O-free `domain -> JMAP session resource` resolver.
pub struct DiscoveryJmapResolve {
    domain: String,
    srv: bool,
    state: State,
}

impl DiscoveryJmapResolve {
    /// Builds a resolver for `domain`. `resolver` is the
    /// `tcp://host:port` DNS-over-TCP resolver URL used for the SRV
    /// lookup.
    pub fn new(domain: impl AsRef<str>, resolver: Url) -> Self {
        let domain = domain.as_ref().trim_matches('.').to_string();
        let srv = DiscoveryDnsSrv::new(format!("_jmap._tcp.{domain}"), resolver);

        Self {
            domain,
            srv: false,
            state: State::Srv(Box::new(srv)),
        }
    }

    fn origin(&self, srv: Option<(String, u16)>) -> Result<Url, DiscoveryJmapResolveError> {
        let (host, port) = srv.unwrap_or_else(|| (self.domain.clone(), 443));

        // RFC 8620 §2.2: the session resource is always HTTPS,
        // whatever the SRV record's port.
        let raw = format!("https://{host}:{port}/");
        Url::parse(&raw).map_err(|err| DiscoveryJmapResolveError::InvalidOrigin(raw, err))
    }

    fn probe(
        &mut self,
        srv: Option<(String, u16)>,
    ) -> DiscoveryCoroutineState<
        DiscoveryYield,
        Result<DiscoveryJmapResolveOutput, DiscoveryJmapResolveError>,
    > {
        self.srv = srv.is_some();

        let origin = match self.origin(srv) {
            Ok(origin) => origin,
            Err(err) => return DiscoveryCoroutineState::Complete(Err(err)),
        };

        let probe = Box::new(DiscoveryJmapWellKnown::new(origin.clone()));
        self.state = State::DiscoveryWellKnown { probe, origin };
        self.resume(None)
    }
}

impl DiscoveryCoroutine for DiscoveryJmapResolve {
    type Yield = DiscoveryYield;
    type Return = Result<DiscoveryJmapResolveOutput, DiscoveryJmapResolveError>;

    fn resume(&mut self, arg: Option<&[u8]>) -> DiscoveryCoroutineState<Self::Yield, Self::Return> {
        match mem::take(&mut self.state) {
            State::Srv(mut srv) => match srv.resume(arg) {
                DiscoveryCoroutineState::Complete(Ok(records)) => {
                    // Best record first (lowest priority, highest
                    // weight), already sorted by the SRV coroutine.
                    let best = records.into_iter().next().map(|record| {
                        let srv = record.rdata;
                        let host = srv.name.to_string().trim_end_matches('.').to_string();
                        (host, srv.port.get())
                    });
                    self.probe(best)
                }
                DiscoveryCoroutineState::Yielded(y) => {
                    self.state = State::Srv(srv);
                    DiscoveryCoroutineState::Yielded(y)
                }
                // NOTE: an unreachable resolver (mobile networks
                // routinely block outbound DNS) must not kill the
                // resolve: proceed as if no SRV record was published
                // and probe `.well-known/jmap` on the domain itself,
                // whose HTTPS socket resolves through the OS.
                DiscoveryCoroutineState::Complete(Err(err)) => {
                    debug!("skip failed SRV lookup, probe .well-known/jmap");
                    trace!("{err:?}");
                    self.probe(None)
                }
            },
            State::DiscoveryWellKnown { mut probe, origin } => match probe.resume(arg) {
                DiscoveryCoroutineState::Complete(Ok(Some(session))) => {
                    let srv = self.srv;
                    let output = DiscoveryJmapResolveOutput { session, srv };
                    DiscoveryCoroutineState::Complete(Ok(output))
                }
                DiscoveryCoroutineState::Complete(Ok(None)) => DiscoveryCoroutineState::Complete(
                    Err(DiscoveryJmapResolveError::NotFound(origin)),
                ),
                DiscoveryCoroutineState::Yielded(y) => {
                    self.state = State::DiscoveryWellKnown { probe, origin };
                    DiscoveryCoroutineState::Yielded(y)
                }
                DiscoveryCoroutineState::Complete(Err(err)) => {
                    DiscoveryCoroutineState::Complete(Err(err.into()))
                }
            },
            State::Done => panic!("DiscoveryJmapResolve::resume called after completion"),
        }
    }
}

#[derive(Default)]
enum State {
    Srv(Box<DiscoveryDnsSrv>),
    DiscoveryWellKnown {
        probe: Box<DiscoveryJmapWellKnown>,
        origin: Url,
    },
    #[default]
    Done,
}

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;

    use url::Url;

    use crate::{
        coroutine::{DiscoveryCoroutine, DiscoveryCoroutineState, DiscoveryYield},
        rfc8620::resolve::{DiscoveryJmapResolve, DiscoveryJmapResolveOutput},
    };

    /// A DNS-over-TCP answer: one SRV record naming `target` on port
    /// 443, or no record.
    fn srv_reply(target: Option<&str>) -> Vec<u8> {
        let count = u8::from(target.is_some());
        let mut message = vec![0, 1, 0x81, 0x80, 0, 0, 0, count, 0, 0, 0, 0];

        if let Some(target) = target {
            let mut rdata = vec![0, 0, 0, 1, 0x01, 0xBB];
            for label in target.split('.') {
                rdata.push(label.len() as u8);
                rdata.extend(label.as_bytes());
            }
            rdata.push(0);

            message.extend([0, 0, 33, 0, 1, 0, 0, 0, 60, 0, rdata.len() as u8]);
            message.extend(rdata);
        }

        let mut reply = (message.len() as u16).to_be_bytes().to_vec();
        reply.extend(message);
        reply
    }

    /// Resolves JMAP on `example.com`, answering the SRV lookup with
    /// `srv` then the session probe with a challenged 401.
    fn resolve(srv: Vec<u8>) -> DiscoveryJmapResolveOutput {
        let resolver = Url::parse("tcp://1.1.1.1:53").unwrap();
        let mut resolve = DiscoveryJmapResolve::new("example.com", resolver);

        let challenge =
            b"HTTP/1.1 401 Unauthorized\r\nWWW-Authenticate: Bearer\r\nContent-Length: 0\r\n\r\n";
        let mut replies = [srv.as_slice(), challenge.as_slice()].into_iter();
        let mut arg = None;

        loop {
            match resolve.resume(arg.take()) {
                DiscoveryCoroutineState::Yielded(DiscoveryYield::WantsWrite { .. }) => {}
                DiscoveryCoroutineState::Yielded(DiscoveryYield::WantsRead { .. }) => {
                    arg = Some(replies.next().expect("a reply for every read"));
                }
                DiscoveryCoroutineState::Complete(res) => return res.unwrap(),
            }
        }
    }

    #[test]
    fn the_output_tells_whether_an_srv_record_named_the_origin() {
        let output = resolve(srv_reply(None));
        assert_eq!(
            output.session.url.as_str(),
            "https://example.com/.well-known/jmap"
        );
        assert!(!output.srv);

        let output = resolve(srv_reply(Some("jmap.example.net")));
        assert_eq!(
            output.session.url.as_str(),
            "https://jmap.example.net/.well-known/jmap"
        );
        assert!(output.srv);
    }
}
