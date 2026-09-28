//! # `.well-known/{caldav,carddav}` probe (RFC 6764 §5)
//!
//! [`DiscoveryWellKnown`] sends the bootstrap `PROPFIND` of
//! [`DiscoveryDavContext`] to `<origin>/.well-known/{service}`, follows
//! the redirect chain, and reports its terminal URL as the DAV context
//! root when the origin redirected at least once and a DAV server
//! answered at the end. A web page at the end of the chain, such as
//! the login form an apex domain bounces `.well-known/*` onto, or an
//! origin that does not redirect, resolves to `None`.
//!
//! Discovery runs unauthenticated by design (RFC 6764 bootstrap); the
//! per-user principal walk that needs credentials lives in the WebDAV
//! client, not here.
//!
//! [`DiscoveryDavContext`]: crate::rfc6764::context::DiscoveryDavContext

use alloc::boxed::Box;

use io_http::rfc8615::well_known::{Http11WellKnown, Http11WellKnownError};
use log::trace;
use thiserror::Error;
use url::Url;

use crate::{
    coroutine::{DiscoveryCoroutine, DiscoveryCoroutineState, DiscoveryYield},
    rfc6764::{
        context::{is_context_root, propfind},
        service::DiscoveryDavService,
    },
    shared::walk::DiscoveryHttpWalk,
};

/// Errors emitted by [`DiscoveryWellKnown`].
#[derive(Debug, Error)]
pub enum DiscoveryWellKnownError {
    /// The underlying HTTP well-known request or response failed.
    #[error(transparent)]
    Http(#[from] Http11WellKnownError),
}

/// I/O-free `.well-known` probe. Yields its current target URL on
/// every step so the std client routes bytes through the matching
/// HTTPS stream, hopping streams when a redirect crosses origins.
/// Completes with the context root, or `None` when the origin did not
/// redirect to a DAV server.
pub struct DiscoveryWellKnown {
    state: State,
}

impl DiscoveryWellKnown {
    /// Builds a probe for `service` against `origin`, a scheme + host
    /// + port root such as `https://carddav.example.com/`.
    pub fn new(origin: Url, service: DiscoveryDavService) -> Self {
        let state = match Http11WellKnown::prepare_request(origin.as_str(), service.service_name())
        {
            Ok(request) => State::Walk(Box::new(DiscoveryHttpWalk::new(propfind(request.url)))),
            Err(err) => State::Failed(Some(err.into())),
        };

        Self { state }
    }
}

impl DiscoveryCoroutine for DiscoveryWellKnown {
    type Yield = DiscoveryYield;
    type Return = Result<Option<Url>, DiscoveryWellKnownError>;

    fn resume(&mut self, arg: Option<&[u8]>) -> DiscoveryCoroutineState<Self::Yield, Self::Return> {
        let walk = match &mut self.state {
            State::Walk(walk) => walk,
            State::Failed(err) => {
                let err = err
                    .take()
                    .expect("DiscoveryWellKnown resumed after completion");
                return DiscoveryCoroutineState::Complete(Err(err));
            }
        };

        match walk.resume(arg) {
            DiscoveryCoroutineState::Yielded(y) => DiscoveryCoroutineState::Yielded(y),
            DiscoveryCoroutineState::Complete(Ok(Some(output))) => {
                let status = *output.response.status;

                if output.hops == 0 {
                    trace!("well-known answered {status} without redirect");
                    return DiscoveryCoroutineState::Complete(Ok(None));
                }

                if !is_context_root(&output.response) {
                    trace!(
                        "well-known led to {}, which answered {status}, not a DAV server",
                        output.url
                    );
                    return DiscoveryCoroutineState::Complete(Ok(None));
                }

                trace!("well-known led to DAV context root {}", output.url);
                DiscoveryCoroutineState::Complete(Ok(Some(output.url)))
            }
            DiscoveryCoroutineState::Complete(Ok(None)) => {
                DiscoveryCoroutineState::Complete(Ok(None))
            }
            DiscoveryCoroutineState::Complete(Err(err)) => {
                DiscoveryCoroutineState::Complete(Err(err.into()))
            }
        }
    }
}

enum State {
    Walk(Box<DiscoveryHttpWalk>),
    Failed(Option<DiscoveryWellKnownError>),
}

#[cfg(test)]
mod tests {
    use alloc::{format, string::String};

    use url::Url;

    use crate::{
        coroutine::{DiscoveryCoroutine, DiscoveryCoroutineState, DiscoveryYield},
        rfc6764::{service::DiscoveryDavService, well_known::DiscoveryWellKnown},
    };

    /// Probes `.well-known/caldav` on `https://example.com/`, answering
    /// each request of the redirect chain with the next of `replies`.
    fn probe(replies: &[String]) -> Option<Url> {
        let origin = Url::parse("https://example.com/").unwrap();
        let mut probe = DiscoveryWellKnown::new(origin, DiscoveryDavService::Caldav);
        let mut state = probe.resume(None);

        for reply in replies {
            let DiscoveryCoroutineState::Yielded(DiscoveryYield::WantsWrite { .. }) = state else {
                panic!("expected a request to be written");
            };

            let DiscoveryCoroutineState::Yielded(DiscoveryYield::WantsRead { .. }) =
                probe.resume(None)
            else {
                panic!("expected the response to be read");
            };

            state = probe.resume(Some(reply.as_bytes()));
        }

        let DiscoveryCoroutineState::Complete(Ok(root)) = state else {
            panic!("expected the probe to complete");
        };

        root
    }

    /// Builds a raw HTTP response out of its head and body.
    fn reply(head: &str, body: &str) -> String {
        format!("{head}\r\nContent-Length: {}\r\n\r\n{body}", body.len())
    }

    #[test]
    fn redirect_chain_is_followed_to_the_dav_server() {
        let root = probe(&[
            reply(
                "HTTP/1.1 301 Moved Permanently\r\nLocation: https://dav.example.com/.well-known/caldav",
                "",
            ),
            reply(
                "HTTP/1.1 301 Moved Permanently\r\nLocation: https://dav.example.com/dav/calendars",
                "",
            ),
            reply(
                "HTTP/1.1 401 Unauthorized\r\nWWW-Authenticate: Basic realm=\"dav\", Bearer",
                "",
            ),
        ]);

        assert_eq!(
            root.unwrap().as_str(),
            "https://dav.example.com/dav/calendars"
        );
    }

    #[test]
    fn redirect_to_a_login_page_is_not_a_context_root() {
        // The ik.me shape.
        let root = probe(&[
            reply(
                "HTTP/1.1 301 Moved Permanently\r\nLocation: https://login.example.com/login",
                "",
            ),
            reply(
                "HTTP/1.1 405 Method Not Allowed\r\nContent-Type: text/html",
                "<html></html>",
            ),
        ]);

        assert!(root.is_none());
    }

    #[test]
    fn origin_without_redirect_is_not_a_context_root() {
        let root = probe(&[reply(
            "HTTP/1.1 401 Unauthorized\r\nWWW-Authenticate: Basic realm=\"dav\"",
            "",
        )]);

        assert!(root.is_none());
    }
}
