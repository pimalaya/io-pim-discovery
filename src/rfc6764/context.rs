//! # DAV context root check (RFC 6764 §6)
//!
//! [`DiscoveryDavContext`] tells whether a URL leads to a CalDAV or
//! CardDAV server before discovery hands it out as a context root. It
//! sends the first request of the RFC 6764 §6 bootstrap, a `PROPFIND`
//! with `Depth: 0` asking for `current-user-principal`, follows any
//! redirect chain, and accepts the terminal URL when the response is a
//! `207 Multi-Status`, or a `401` carrying a `WWW-Authenticate`
//! challenge (discovery runs unauthenticated, so the credentials a DAV
//! server asks for are the sign of it). A web page answers the method
//! with a 405 or a 200, and is rejected.

use alloc::{string::ToString, vec};

use io_http::{
    rfc8615::well_known::Http11WellKnownError,
    rfc9110::{request::HttpRequest, response::HttpResponse},
};
use log::trace;
use thiserror::Error;
use url::Url;

use crate::{
    coroutine::{DiscoveryCoroutine, DiscoveryCoroutineState, DiscoveryYield},
    shared::walk::DiscoveryHttpWalk,
};

/// Body of the bootstrap `PROPFIND` (RFC 6764 §6, RFC 5397).
const PROPFIND: &str = r#"<?xml version="1.0" encoding="utf-8"?><propfind xmlns="DAV:"><prop><current-user-principal/></prop></propfind>"#;

/// Errors emitted by [`DiscoveryDavContext`].
#[derive(Debug, Error)]
pub enum DiscoveryDavContextError {
    /// The underlying HTTP request or response failed.
    #[error(transparent)]
    Http(#[from] Http11WellKnownError),
}

/// I/O-free DAV context root check. Yields its current target URL on
/// every step so the std client routes bytes through the matching
/// stream. Completes with the terminal URL of the redirect chain, or
/// `None` when no DAV server answered there.
pub struct DiscoveryDavContext {
    walk: DiscoveryHttpWalk,
}

impl DiscoveryDavContext {
    /// Builds a check of `url`, a candidate context root.
    pub fn new(url: Url) -> Self {
        Self {
            walk: DiscoveryHttpWalk::new(propfind(url)),
        }
    }
}

impl DiscoveryCoroutine for DiscoveryDavContext {
    type Yield = DiscoveryYield;
    type Return = Result<Option<Url>, DiscoveryDavContextError>;

    fn resume(&mut self, arg: Option<&[u8]>) -> DiscoveryCoroutineState<Self::Yield, Self::Return> {
        match self.walk.resume(arg) {
            DiscoveryCoroutineState::Yielded(y) => DiscoveryCoroutineState::Yielded(y),
            DiscoveryCoroutineState::Complete(Ok(Some(output)))
                if is_context_root(&output.response) =>
            {
                trace!("DAV server answered at {}", output.url);
                DiscoveryCoroutineState::Complete(Ok(Some(output.url)))
            }
            DiscoveryCoroutineState::Complete(Ok(Some(output))) => {
                let status = *output.response.status;
                trace!("{} answered {status}, not a DAV server", output.url);
                DiscoveryCoroutineState::Complete(Ok(None))
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

/// Builds the bootstrap `PROPFIND` against `url`.
pub(crate) fn propfind(url: Url) -> HttpRequest {
    HttpRequest {
        method: "PROPFIND".to_string(),
        url,
        headers: vec![
            ("Depth".to_string(), "0".to_string()),
            (
                "Content-Type".to_string(),
                "application/xml; charset=utf-8".to_string(),
            ),
        ],
        body: PROPFIND.as_bytes().to_vec(),
    }
}

/// Whether `response` to the bootstrap `PROPFIND` comes from a DAV
/// server: a `207 Multi-Status`, or a `401` carrying a challenge.
pub(crate) fn is_context_root(response: &HttpResponse) -> bool {
    match *response.status {
        207 => true,
        401 => !response.challenges().is_empty(),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use alloc::{format, string::String};

    use url::Url;

    use crate::{
        coroutine::{DiscoveryCoroutine, DiscoveryCoroutineState, DiscoveryYield},
        rfc6764::context::DiscoveryDavContext,
    };

    /// Checks `https://dav.example.com/dav/` against `reply`, asserting
    /// the request is the bootstrap `PROPFIND`.
    fn check(reply: &str) -> Option<Url> {
        let url = Url::parse("https://dav.example.com/dav/").unwrap();
        let mut check = DiscoveryDavContext::new(url);

        let DiscoveryCoroutineState::Yielded(DiscoveryYield::WantsWrite { bytes, .. }) =
            check.resume(None)
        else {
            panic!("expected the request to be written");
        };

        let request = String::from_utf8(bytes).unwrap();
        assert!(request.starts_with("PROPFIND /dav/ HTTP/1.1\r\n"));
        assert!(request.contains("Depth: 0\r\n"));
        assert!(request.contains("<current-user-principal/>"));

        let DiscoveryCoroutineState::Yielded(DiscoveryYield::WantsRead { .. }) = check.resume(None)
        else {
            panic!("expected the response to be read");
        };

        let DiscoveryCoroutineState::Complete(Ok(root)) = check.resume(Some(reply.as_bytes()))
        else {
            panic!("expected the check to complete");
        };

        root
    }

    /// Builds a raw HTTP response out of its head and body.
    fn reply(head: &str, body: &str) -> String {
        format!("{head}\r\nContent-Length: {}\r\n\r\n{body}", body.len())
    }

    #[test]
    fn multi_status_is_a_dav_server() {
        let multistatus = reply(
            "HTTP/1.1 207 Multi-Status\r\nContent-Type: application/xml",
            r#"<multistatus xmlns="DAV:"/>"#,
        );

        let root = check(&multistatus).unwrap();
        assert_eq!(root.as_str(), "https://dav.example.com/dav/");
    }

    #[test]
    fn challenged_401_is_a_dav_server() {
        let challenge = reply(
            "HTTP/1.1 401 Unauthorized\r\nWWW-Authenticate: Basic realm=\"sabre/dav\"",
            "",
        );

        assert!(check(&challenge).is_some());
    }

    #[test]
    fn unchallenged_401_is_not_a_dav_server() {
        let portal = reply("HTTP/1.1 401 Unauthorized", "Unauthorized");

        assert!(check(&portal).is_none());
    }

    #[test]
    fn web_page_rejecting_propfind_is_not_a_dav_server() {
        // The ik.me login form and example.com answer 405.
        let page = reply(
            "HTTP/1.1 405 Method Not Allowed\r\nContent-Type: text/html",
            "<html></html>",
        );

        assert!(check(&page).is_none());
    }
}
