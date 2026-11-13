//! # Combined RFC 6764 resolve coroutine
//!
//! [`DiscoveryDavResolve`] turns a bare domain into a CalDAV/CardDAV context
//! root by chaining the three RFC 6764 mechanisms:
//!
//! 1. SRV lookups (§3), picking the best record for the requested
//!    service (secure `_caldavs`/`_carddavs` first, plain
//!    `_caldav`/`_carddav` next) to fix the origin host and port.
//! 2. A TXT `path` lookup (§4) on that same SRV name: when present,
//!    its path joined onto the origin is the first candidate. A
//!    failed lookup counts as no path.
//! 3. A `.well-known` probe (§5) on the origin, following the
//!    context-path redirect chain.
//! 4. The origin itself.
//!
//! Every candidate is checked by [`DiscoveryDavContext`] and the first
//! one leading to a DAV server wins; when none does, the resolve fails
//! with [`DiscoveryDavResolveError::NotFound`]. When the domain
//! publishes no SRV record the origin is `https://<domain>:443/`.
//!
//! Composing the DNS steps (over the `tcp://` resolver) and the HTTPS
//! well-known step in one coroutine works because every yield carries
//! its endpoint URL: the std client routes each step through the
//! matching stream in its [`DiscoveryStreamPool`].
//!
//! [`DiscoveryDavContext`]: crate::rfc6764::context::DiscoveryDavContext
//! [`DiscoveryStreamPool`]: crate::shared::pool::DiscoveryStreamPool

use core::mem;

use alloc::{
    format,
    string::{String, ToString},
};

use log::{debug, trace};
use thiserror::Error;
use url::{Host, Url};

use crate::{
    coroutine::{DiscoveryCoroutine, DiscoveryCoroutineState, DiscoveryYield},
    rfc6186::service::DiscoverySrvService,
    rfc6764::{
        context::{DiscoveryDavContext, DiscoveryDavContextError},
        discover::{DiscoveryWebdavSrv, DiscoveryWebdavSrvError},
        service::DiscoveryDavService,
        txt::{DiscoveryWebdavTxt, DiscoveryWebdavTxtError},
        well_known::{DiscoveryWellKnown, DiscoveryWellKnownError},
    },
};

/// Errors emitted by [`DiscoveryDavResolve`].
#[derive(Debug, Error)]
pub enum DiscoveryDavResolveError {
    /// One of the four RFC 6764 §3 SRV lookups failed.
    #[error(transparent)]
    Srv(#[from] DiscoveryWebdavSrvError),
    /// The RFC 6764 §4 TXT `path` lookup failed.
    #[error(transparent)]
    Txt(#[from] DiscoveryWebdavTxtError),
    /// The RFC 6764 §5 `.well-known` probe failed.
    #[error(transparent)]
    DiscoveryWellKnown(#[from] DiscoveryWellKnownError),
    /// Checking a candidate context root failed.
    #[error(transparent)]
    Context(#[from] DiscoveryDavContextError),
    /// The scheme + host + port assembled from SRV records did not
    /// form a valid URL, or the host is not a host name.
    #[error("RFC 6764 resolve built an invalid origin URL `{0}`: {1}")]
    InvalidOrigin(String, #[source] url::ParseError),
    /// Joining the TXT `path` value onto the origin URL failed.
    #[error("RFC 6764 resolve could not apply TXT path `{0}`: {1}")]
    InvalidPath(String, #[source] url::ParseError),
    /// No candidate context root led to a DAV server.
    #[error("RFC 6764 resolve found no DAV server on `{0}`")]
    NotFound(Url),
}

/// The context root found by a [`DiscoveryDavResolve`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscoveryDavResolveOutput {
    /// The DAV context root.
    pub root: Url,
    /// Whether an SRV record named the origin, not the domain itself.
    pub srv: bool,
}

/// I/O-free `domain -> DAV context root` resolver.
pub struct DiscoveryDavResolve {
    service: DiscoveryDavService,
    domain: String,
    resolver: Url,
    secure_only: bool,
    srv: bool,
    state: State,
}

impl DiscoveryDavResolve {
    /// Builds a resolver for `service` on `domain`. `resolver` is the
    /// `tcp://host:port` DNS-over-TCP resolver URL used for the SRV
    /// and TXT lookups.
    pub fn new(domain: impl AsRef<str>, service: DiscoveryDavService, resolver: Url) -> Self {
        let domain = domain.as_ref().trim_matches('.').to_string();
        let srv = DiscoveryWebdavSrv::new(&domain, resolver.clone());

        Self {
            service,
            domain,
            resolver,
            secure_only: false,
            srv: false,
            state: State::Srv(srv),
        }
    }

    /// Ignores the plain `_caldav` / `_carddav` record when
    /// `secure_only` is set, so that no `http://` origin is built: the
    /// origin is then the TLS record's, or `https` on the domain.
    pub fn with_secure_only(mut self, secure_only: bool) -> Self {
        self.secure_only = secure_only;
        self
    }

    /// Picks the SRV record fixing the origin, paired with whether it
    /// is the TLS one: the TLS record first, then the plain one unless
    /// secure-only.
    fn record(
        &self,
        secure: Option<DiscoverySrvService>,
        plain: Option<DiscoverySrvService>,
    ) -> Option<(bool, DiscoverySrvService)> {
        match (secure, plain) {
            (Some(secure), _) => Some((true, secure)),
            (None, Some(plain)) if !self.secure_only => Some((false, plain)),
            _ => None,
        }
    }

    fn origin(
        &self,
        record: Option<&(bool, DiscoverySrvService)>,
    ) -> Result<Url, DiscoveryDavResolveError> {
        let (scheme, host, port) = match record {
            Some((true, srv)) => ("https", srv.host.as_str(), srv.port),
            Some((false, srv)) => ("http", srv.host.as_str(), srv.port),
            None => ("https", self.domain.as_str(), 443),
        };

        let raw = format!("{scheme}://{host}:{port}/");

        if let Err(err) = Host::parse(host) {
            return Err(DiscoveryDavResolveError::InvalidOrigin(raw, err));
        }

        Url::parse(&raw).map_err(|err| DiscoveryDavResolveError::InvalidOrigin(raw, err))
    }
}

impl DiscoveryCoroutine for DiscoveryDavResolve {
    type Yield = DiscoveryYield;
    type Return = Result<DiscoveryDavResolveOutput, DiscoveryDavResolveError>;

    fn resume(&mut self, arg: Option<&[u8]>) -> DiscoveryCoroutineState<Self::Yield, Self::Return> {
        match mem::take(&mut self.state) {
            State::Srv(mut srv) => match srv.resume(arg) {
                DiscoveryCoroutineState::Complete(Ok(report)) => {
                    let (secure, plain) = match self.service {
                        DiscoveryDavService::Caldav => (report.caldavs, report.caldav),
                        DiscoveryDavService::Carddav => (report.carddavs, report.carddav),
                    };

                    let record = self.record(secure, plain);
                    self.srv = record.is_some();

                    // RFC 6764 §6 only queries the TXT `path` record for
                    // a name that actually published an SRV record; with
                    // no SRV record there is no service name to look up.
                    let txt_qname = record
                        .as_ref()
                        .map(|(tls, _)| self.service.srv_qname(*tls, &self.domain));

                    let origin = match self.origin(record.as_ref()) {
                        Ok(origin) => origin,
                        Err(err) => return DiscoveryCoroutineState::Complete(Err(err)),
                    };

                    self.state = match txt_qname {
                        Some(qname) => {
                            let txt = DiscoveryWebdavTxt::new(qname, self.resolver.clone());
                            State::Txt { txt, origin }
                        }
                        None => {
                            let probe = DiscoveryWellKnown::new(origin.clone(), self.service);
                            State::DiscoveryWellKnown { probe, origin }
                        }
                    };
                    self.resume(None)
                }
                DiscoveryCoroutineState::Yielded(y) => {
                    self.state = State::Srv(srv);
                    DiscoveryCoroutineState::Yielded(y)
                }
                // NOTE: an unreachable resolver (mobile networks
                // routinely block outbound DNS) must not kill the
                // resolve: proceed as if no SRV record was published
                // and probe `.well-known` on the domain itself, whose
                // HTTPS socket resolves through the OS.
                DiscoveryCoroutineState::Complete(Err(err)) => {
                    debug!("skip failed SRV lookups, probe .well-known");
                    trace!("{err:?}");

                    let origin = match self.origin(None) {
                        Ok(origin) => origin,
                        Err(err) => return DiscoveryCoroutineState::Complete(Err(err)),
                    };

                    let probe = DiscoveryWellKnown::new(origin.clone(), self.service);
                    self.state = State::DiscoveryWellKnown { probe, origin };
                    self.resume(None)
                }
            },
            State::Txt { mut txt, origin } => match txt.resume(arg) {
                DiscoveryCoroutineState::Complete(Ok(Some(path))) => match origin.join(&path) {
                    Ok(root) => {
                        let check = DiscoveryDavContext::new(root);
                        self.state = State::Context {
                            check,
                            origin,
                            candidate: Candidate::TxtPath,
                        };
                        self.resume(None)
                    }
                    Err(err) => DiscoveryCoroutineState::Complete(Err(
                        DiscoveryDavResolveError::InvalidPath(path, err),
                    )),
                },
                DiscoveryCoroutineState::Complete(Ok(None)) => {
                    let probe = DiscoveryWellKnown::new(origin.clone(), self.service);
                    self.state = State::DiscoveryWellKnown { probe, origin };
                    self.resume(None)
                }
                DiscoveryCoroutineState::Yielded(y) => {
                    self.state = State::Txt { txt, origin };
                    DiscoveryCoroutineState::Yielded(y)
                }
                DiscoveryCoroutineState::Complete(Err(err)) => {
                    debug!("skip failed TXT lookup, probe .well-known");
                    trace!("{err:?}");

                    let probe = DiscoveryWellKnown::new(origin.clone(), self.service);
                    self.state = State::DiscoveryWellKnown { probe, origin };
                    self.resume(None)
                }
            },
            State::DiscoveryWellKnown { mut probe, origin } => match probe.resume(arg) {
                DiscoveryCoroutineState::Complete(Ok(Some(root))) => {
                    let srv = self.srv;
                    DiscoveryCoroutineState::Complete(Ok(DiscoveryDavResolveOutput { root, srv }))
                }
                DiscoveryCoroutineState::Complete(Ok(None)) => {
                    let check = DiscoveryDavContext::new(origin.clone());
                    self.state = State::Context {
                        check,
                        origin,
                        candidate: Candidate::Origin,
                    };
                    self.resume(None)
                }
                DiscoveryCoroutineState::Yielded(y) => {
                    self.state = State::DiscoveryWellKnown { probe, origin };
                    DiscoveryCoroutineState::Yielded(y)
                }
                DiscoveryCoroutineState::Complete(Err(err)) => {
                    DiscoveryCoroutineState::Complete(Err(err.into()))
                }
            },
            State::Context {
                mut check,
                origin,
                candidate,
            } => match check.resume(arg) {
                DiscoveryCoroutineState::Complete(Ok(Some(root))) => {
                    let srv = self.srv;
                    DiscoveryCoroutineState::Complete(Ok(DiscoveryDavResolveOutput { root, srv }))
                }
                DiscoveryCoroutineState::Complete(Ok(None)) => match candidate {
                    Candidate::TxtPath => {
                        debug!("skip TXT path leading to no DAV server, probe .well-known");
                        let probe = DiscoveryWellKnown::new(origin.clone(), self.service);
                        self.state = State::DiscoveryWellKnown { probe, origin };
                        self.resume(None)
                    }
                    Candidate::Origin => DiscoveryCoroutineState::Complete(Err(
                        DiscoveryDavResolveError::NotFound(origin),
                    )),
                },
                DiscoveryCoroutineState::Yielded(y) => {
                    self.state = State::Context {
                        check,
                        origin,
                        candidate,
                    };
                    DiscoveryCoroutineState::Yielded(y)
                }
                DiscoveryCoroutineState::Complete(Err(err)) => {
                    DiscoveryCoroutineState::Complete(Err(err.into()))
                }
            },
            State::Done => panic!("DiscoveryDavResolve::resume called after completion"),
        }
    }
}

#[derive(Default)]
enum State {
    Srv(DiscoveryWebdavSrv),
    Txt {
        txt: DiscoveryWebdavTxt,
        origin: Url,
    },
    DiscoveryWellKnown {
        probe: DiscoveryWellKnown,
        origin: Url,
    },
    Context {
        check: DiscoveryDavContext,
        origin: Url,
        candidate: Candidate,
    },
    #[default]
    Done,
}

/// The candidate context root a [`State::Context`] check is on, which
/// decides what comes next when it leads to no DAV server.
#[derive(Clone, Copy)]
enum Candidate {
    /// The TXT `path` joined onto the origin; `.well-known` is next.
    TxtPath,
    /// The origin itself, the last resort.
    Origin,
}

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;

    use url::Url;

    use crate::{
        coroutine::{DiscoveryCoroutine, DiscoveryCoroutineState, DiscoveryYield},
        rfc6186::service::DiscoverySrvService,
        rfc6764::{
            resolve::{DiscoveryDavResolve, DiscoveryDavResolveOutput},
            service::DiscoveryDavService,
        },
    };

    fn origin(secure_only: bool) -> Url {
        let resolver = Url::parse("tcp://1.1.1.1:53").unwrap();
        let resolve =
            DiscoveryDavResolve::new("example.com", DiscoveryDavService::Caldav, resolver)
                .with_secure_only(secure_only);

        let plain = DiscoverySrvService {
            host: "dav.example.com".into(),
            port: 8080,
            priority: 0,
            weight: 1,
        };

        let record = resolve.record(None, Some(plain));
        resolve.origin(record.as_ref()).unwrap()
    }

    #[test]
    fn plain_record_builds_an_http_origin() {
        assert_eq!(origin(false).as_str(), "http://dav.example.com:8080/");
    }

    #[test]
    fn secure_only_ignores_the_plain_record() {
        assert_eq!(origin(true).as_str(), "https://example.com/");
    }

    #[test]
    fn a_domain_that_is_not_a_host_name_builds_no_origin() {
        let resolver = Url::parse("tcp://1.1.1.1:53").unwrap();
        let service = DiscoveryDavService::Caldav;
        let resolve = DiscoveryDavResolve::new("example.org:8443/x?", service, resolver);

        assert!(resolve.origin(None).is_err());
    }

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

    /// Resolves CalDAV on `example.com`, answering the four SRV lookups
    /// with `srv` then every further read with the next of `replies`.
    fn resolve(srv: [Vec<u8>; 4], replies: &[Vec<u8>]) -> DiscoveryDavResolveOutput {
        let resolver = Url::parse("tcp://1.1.1.1:53").unwrap();
        let mut resolve =
            DiscoveryDavResolve::new("example.com", DiscoveryDavService::Caldav, resolver);

        let mut replies = srv.iter().chain(replies);
        let mut arg = None;

        loop {
            match resolve.resume(arg.take()) {
                DiscoveryCoroutineState::Yielded(DiscoveryYield::WantsWrite { .. }) => {}
                DiscoveryCoroutineState::Yielded(DiscoveryYield::WantsRead { .. }) => {
                    let reply = replies.next().expect("a reply for every read");
                    arg = Some(reply.as_slice());
                }
                DiscoveryCoroutineState::Complete(res) => return res.unwrap(),
            }
        }
    }

    #[test]
    fn the_output_tells_whether_an_srv_record_named_the_origin() {
        let redirect =
            b"HTTP/1.1 301 Moved Permanently\r\nLocation: /dav/\r\nContent-Length: 0\r\n\r\n";
        let challenge = b"HTTP/1.1 401 Unauthorized\r\nWWW-Authenticate: Basic realm=\"dav\"\r\nContent-Length: 0\r\n\r\n";
        let http = [redirect.to_vec(), challenge.to_vec()];

        let none = || srv_reply(None);
        let output = resolve([none(), none(), none(), none()], &http);
        assert_eq!(output.root.as_str(), "https://example.com/dav/");
        assert!(!output.srv);

        // The `_caldavs` record, then no TXT path.
        let named = srv_reply(Some("dav.example.net"));
        let replies = [none(), redirect.to_vec(), challenge.to_vec()];
        let output = resolve([none(), named, none(), none()], &replies);
        assert_eq!(output.root.as_str(), "https://dav.example.net/dav/");
        assert!(output.srv);
    }

    #[test]
    fn a_failed_txt_lookup_counts_as_no_path() {
        let redirect =
            b"HTTP/1.1 301 Moved Permanently\r\nLocation: /dav/\r\nContent-Length: 0\r\n\r\n";
        let challenge = b"HTTP/1.1 401 Unauthorized\r\nWWW-Authenticate: Basic realm=\"dav\"\r\nContent-Length: 0\r\n\r\n";

        // The `_caldavs` record, then a TXT stream closing unanswered.
        let none = || srv_reply(None);
        let named = srv_reply(Some("dav.example.net"));
        let replies = [Vec::new(), redirect.to_vec(), challenge.to_vec()];
        let output = resolve([none(), named, none(), none()], &replies);

        assert_eq!(output.root.as_str(), "https://dav.example.net/dav/");
    }
}
