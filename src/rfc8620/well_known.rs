//! # `.well-known/jmap` session probe (RFC 8620 §2.2)
//!
//! [`DiscoveryJmapWellKnown`] GETs `<origin>/.well-known/jmap` and follows any
//! redirect chain (RFC 8620 locates the session resource "following
//! any redirects"; apex domains routinely bounce through marketing
//! hosts), then validates the terminal response before reporting it
//! as the JMAP Session resource. A 2xx qualifies when its body is a
//! session object (RFC 8620 §2: `capabilities` holding the core
//! capability, and `apiUrl`), whatever its content type. A 401
//! qualifies when it carries a `WWW-Authenticate` challenge: discovery
//! runs unauthenticated by design, so the credentials it asks for are
//! the only sign of the session. Anything else, including the HTML
//! login page an apex domain bounces `.well-known/*` onto, resolves to
//! `None` (no JMAP behind that origin). It reuses the generic RFC 8615
//! probe from [`io_http`] and only adds the redirect walk, the
//! validation and the [`DiscoveryYield`] plumbing so the std client
//! can route each hop through the matching stream.

use alloc::{string::String, vec::Vec};

use io_http::{
    coroutine::{HttpCoroutine, HttpCoroutineState, HttpYield},
    rfc8615::well_known::{Http11WellKnown, Http11WellKnownError},
    rfc9110::request::HttpRequest,
};
use log::trace;
use serde_json::Value;
use thiserror::Error;
use url::Url;

use crate::{
    coroutine::{DiscoveryCoroutine, DiscoveryCoroutineState, DiscoveryYield},
    rfc9110::auth_schemes,
};

/// Redirect hops followed before giving up on a looping chain.
const MAX_HOPS: u8 = 5;

/// The capability every JMAP session advertises (RFC 8620 §2).
const JMAP_CORE: &str = "urn:ietf:params:jmap:core";

/// Errors emitted by [`DiscoveryJmapWellKnown`].
#[derive(Debug, Error)]
pub enum DiscoveryJmapWellKnownError {
    /// The underlying well-known HTTP probe failed.
    #[error(transparent)]
    Http(#[from] Http11WellKnownError),
}

/// The JMAP Session resource located by the probe: its URL and the
/// lowercased authentication schemes its unauthenticated 401
/// advertised through `WWW-Authenticate` (empty when the terminal
/// response was a 2xx, or advertised nothing).
#[derive(Clone, Debug)]
pub struct DiscoveryJmapSessionResource {
    /// URL of the JMAP Session resource (RFC 8620 §2).
    pub url: Url,
    /// Lowercased auth-scheme names from the terminal `WWW-Authenticate`
    /// response (empty on 2xx or when no schemes were advertised).
    pub auth_schemes: Vec<String>,
}

/// I/O-free `.well-known/jmap` session probe. Yields its current
/// target URL on every step so the std client routes bytes through
/// the matching HTTPS stream, hopping streams when a redirect crosses
/// origins. Completes with the session resource, or `None` when the
/// origin serves no JMAP.
pub struct DiscoveryJmapWellKnown {
    target: Url,
    hops: u8,
    probe: Http11WellKnown,
}

impl DiscoveryJmapWellKnown {
    /// Builds a probe against `origin`, a scheme + host + port root
    /// such as `https://api.example.com/`.
    pub fn new(origin: Url) -> Self {
        let mut target = origin;
        target.set_path("/.well-known/jmap");
        Self::request(target, 0)
    }

    /// One GET of the redirect walk, against `target`.
    fn request(target: Url, hops: u8) -> Self {
        let request = HttpRequest::get(target.clone()).header("Accept", "application/json");
        let probe = Http11WellKnown::new(request);

        Self {
            target,
            hops,
            probe,
        }
    }
}

impl DiscoveryCoroutine for DiscoveryJmapWellKnown {
    type Yield = DiscoveryYield;
    type Return = Result<Option<DiscoveryJmapSessionResource>, DiscoveryJmapWellKnownError>;

    fn resume(&mut self, arg: Option<&[u8]>) -> DiscoveryCoroutineState<Self::Yield, Self::Return> {
        match self.probe.resume(arg) {
            HttpCoroutineState::Yielded(HttpYield::WantsRead) => {
                DiscoveryCoroutineState::Yielded(DiscoveryYield::WantsRead {
                    url: self.target.clone(),
                })
            }
            HttpCoroutineState::Yielded(HttpYield::WantsWrite(bytes)) => {
                DiscoveryCoroutineState::Yielded(DiscoveryYield::WantsWrite {
                    url: self.target.clone(),
                    bytes,
                })
            }
            HttpCoroutineState::Complete(Ok(output)) => match output.redirect_url {
                Some(next) => {
                    if self.hops >= MAX_HOPS {
                        trace!("well-known jmap redirected more than {MAX_HOPS} times, give up");
                        return DiscoveryCoroutineState::Complete(Ok(None));
                    }

                    let same_origin = output.same_origin;
                    trace!("well-known jmap redirected to {next} (same origin: {same_origin})");
                    *self = Self::request(next, self.hops + 1);
                    self.resume(None)
                }
                None => {
                    let status = *output.response.status;
                    let auth_schemes = auth_schemes(&output.response);

                    let session = match status {
                        200..=299 => is_session(&output.response.body),
                        401 => !auth_schemes.is_empty(),
                        _ => false,
                    };

                    if !session {
                        trace!(
                            "well-known jmap answered {status} without a session, no JMAP behind this origin"
                        );
                        return DiscoveryCoroutineState::Complete(Ok(None));
                    }

                    trace!(
                        "well-known jmap answered {status}, session lives at {} (schemes {auth_schemes:?})",
                        self.target
                    );

                    DiscoveryCoroutineState::Complete(Ok(Some(DiscoveryJmapSessionResource {
                        url: self.target.clone(),
                        auth_schemes,
                    })))
                }
            },
            HttpCoroutineState::Complete(Err(err)) => {
                DiscoveryCoroutineState::Complete(Err(err.into()))
            }
        }
    }
}

/// Whether `body` is a JMAP Session object (RFC 8620 §2): a JSON
/// object whose `capabilities` hold the core capability, with an
/// `apiUrl` string.
fn is_session(body: &[u8]) -> bool {
    let Ok(session) = serde_json::from_slice::<Value>(body) else {
        return false;
    };

    let core = session
        .get("capabilities")
        .and_then(Value::as_object)
        .is_some_and(|capabilities| capabilities.contains_key(JMAP_CORE));

    core && session.get("apiUrl").is_some_and(Value::is_string)
}

#[cfg(test)]
mod tests {
    use alloc::{format, string::String, vec};

    use url::Url;

    use crate::{
        coroutine::{DiscoveryCoroutine, DiscoveryCoroutineState, DiscoveryYield},
        rfc8620::well_known::{
            DiscoveryJmapSessionResource, DiscoveryJmapWellKnown, DiscoveryJmapWellKnownError,
        },
    };

    type State = DiscoveryCoroutineState<
        DiscoveryYield,
        Result<Option<DiscoveryJmapSessionResource>, DiscoveryJmapWellKnownError>,
    >;

    const SESSION: &str = r#"{"capabilities":{"urn:ietf:params:jmap:core":{}},"apiUrl":"https://jmap.example.com/api/"}"#;

    /// Starts a probe against `https://example.com/`, checking the
    /// request asks for JSON.
    fn probe() -> DiscoveryJmapWellKnown {
        let mut probe = DiscoveryJmapWellKnown::new(Url::parse("https://example.com/").unwrap());

        let DiscoveryCoroutineState::Yielded(DiscoveryYield::WantsWrite { bytes, .. }) =
            probe.resume(None)
        else {
            panic!("expected the request to be written");
        };

        let request = String::from_utf8(bytes).unwrap().to_ascii_lowercase();
        assert!(request.contains("accept: application/json\r\n"));

        probe
    }

    /// Feeds `reply` as the response to the pending request.
    fn answer(probe: &mut DiscoveryJmapWellKnown, reply: &str) -> State {
        let DiscoveryCoroutineState::Yielded(DiscoveryYield::WantsRead { .. }) = probe.resume(None)
        else {
            panic!("expected the response to be read");
        };

        probe.resume(Some(reply.as_bytes()))
    }

    /// Unwraps the resolved session of a completed probe.
    fn resolved(state: State) -> Option<DiscoveryJmapSessionResource> {
        let DiscoveryCoroutineState::Complete(Ok(session)) = state else {
            panic!("expected the probe to complete");
        };

        session
    }

    /// Builds a raw HTTP response out of its head and body.
    fn reply(head: &str, body: &str) -> String {
        format!("{head}\r\nContent-Length: {}\r\n\r\n{body}", body.len())
    }

    #[test]
    fn html_login_page_is_not_a_session() {
        // The ik.me shape: `.well-known/jmap` lands on a login form.
        let mut probe = probe();
        let html = reply(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html",
            "<html>login</html>",
        );

        assert!(resolved(answer(&mut probe, &html)).is_none());
    }

    #[test]
    fn json_without_core_capability_is_not_a_session() {
        let mut probe = probe();
        let json = reply(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json",
            r#"{"capabilities":{},"apiUrl":"https://jmap.example.com/api/"}"#,
        );

        assert!(resolved(answer(&mut probe, &json)).is_none());
    }

    #[test]
    fn session_object_is_a_session_whatever_its_content_type() {
        let mut probe = probe();
        let json = reply("HTTP/1.1 200 OK\r\nContent-Type: text/plain", SESSION);

        let session = resolved(answer(&mut probe, &json)).unwrap();
        assert_eq!(session.url.as_str(), "https://example.com/.well-known/jmap");
        assert!(session.auth_schemes.is_empty());
    }

    #[test]
    fn unchallenged_401_is_not_a_session() {
        let mut probe = probe();
        let portal = reply(
            "HTTP/1.1 401 Unauthorized\r\nContent-Type: text/plain",
            "Unauthorized",
        );

        assert!(resolved(answer(&mut probe, &portal)).is_none());
    }

    #[test]
    fn challenged_401_is_a_session() {
        // The Fastmail shape: a plain-text 401 with a Bearer challenge.
        let mut probe = probe();
        let challenge = reply(
            "HTTP/1.1 401 Unauthorized\r\nContent-Type: text/plain\r\nWWW-Authenticate: Bearer resource_metadata=\"https://example.com/meta\"",
            "No Authorization header",
        );

        let session = resolved(answer(&mut probe, &challenge)).unwrap();
        assert_eq!(session.auth_schemes, vec![String::from("bearer")]);
    }

    #[test]
    fn session_is_validated_at_the_end_of_the_redirect_chain() {
        let mut probe = probe();
        let redirect = reply(
            "HTTP/1.1 301 Moved Permanently\r\nLocation: https://jmap.example.com/session",
            "",
        );

        let DiscoveryCoroutineState::Yielded(DiscoveryYield::WantsWrite { url, .. }) =
            answer(&mut probe, &redirect)
        else {
            panic!("expected the redirect to be followed");
        };
        assert_eq!(url.as_str(), "https://jmap.example.com/session");

        let json = reply("HTTP/1.1 200 OK\r\nContent-Type: application/json", SESSION);
        let session = resolved(answer(&mut probe, &json)).unwrap();
        assert_eq!(session.url.as_str(), "https://jmap.example.com/session");
    }
}
