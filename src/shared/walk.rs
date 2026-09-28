//! # HTTP redirect walk
//!
//! [`DiscoveryHttpWalk`] sends one request and replays it on every
//! redirect target until a response is not a redirect, the way the
//! bootstrap mechanisms locate a resource (RFC 6764 §5, RFC 8620 §2.2:
//! "following any redirects"). It reports the terminal URL and
//! response and leaves judging them to its caller.

use io_http::{
    coroutine::{HttpCoroutine, HttpCoroutineState, HttpYield},
    rfc8615::well_known::{Http11WellKnown, Http11WellKnownError},
    rfc9110::{request::HttpRequest, response::HttpResponse},
};
use log::trace;
use url::Url;

use crate::coroutine::{DiscoveryCoroutine, DiscoveryCoroutineState, DiscoveryYield};

/// Redirect hops followed before giving up on a looping chain.
const MAX_HOPS: u8 = 5;

/// The end of a redirect walk.
#[derive(Clone, Debug)]
pub struct DiscoveryHttpWalkOutput {
    /// URL the terminal response came from.
    pub url: Url,
    /// Redirects followed to reach it.
    pub hops: u8,
    /// The terminal, non-redirect response.
    pub response: HttpResponse,
}

/// I/O-free redirect walk. Yields its current target URL on every
/// step so the std client routes bytes through the matching stream,
/// hopping streams when a redirect crosses origins. Completes with the
/// terminal response, or `None` when the chain looped.
pub struct DiscoveryHttpWalk {
    request: HttpRequest,
    hops: u8,
    send: Http11WellKnown,
}

impl DiscoveryHttpWalk {
    /// Builds a walk starting with `request`, replayed as is, only its
    /// URL changing, on every redirect.
    pub fn new(request: HttpRequest) -> Self {
        let send = Http11WellKnown::new(request.clone());

        Self {
            request,
            hops: 0,
            send,
        }
    }
}

impl DiscoveryCoroutine for DiscoveryHttpWalk {
    type Yield = DiscoveryYield;
    type Return = Result<Option<DiscoveryHttpWalkOutput>, Http11WellKnownError>;

    fn resume(&mut self, arg: Option<&[u8]>) -> DiscoveryCoroutineState<Self::Yield, Self::Return> {
        match self.send.resume(arg) {
            HttpCoroutineState::Yielded(HttpYield::WantsRead) => {
                DiscoveryCoroutineState::Yielded(DiscoveryYield::WantsRead {
                    url: self.request.url.clone(),
                })
            }
            HttpCoroutineState::Yielded(HttpYield::WantsWrite(bytes)) => {
                DiscoveryCoroutineState::Yielded(DiscoveryYield::WantsWrite {
                    url: self.request.url.clone(),
                    bytes,
                })
            }
            HttpCoroutineState::Complete(Ok(output)) => match output.redirect_url {
                Some(next) => {
                    if self.hops >= MAX_HOPS {
                        trace!("redirected more than {MAX_HOPS} times, give up");
                        return DiscoveryCoroutineState::Complete(Ok(None));
                    }

                    let same_origin = output.same_origin;
                    trace!("redirected to {next} (same origin: {same_origin})");

                    self.request.url = next;
                    self.hops += 1;
                    self.send = Http11WellKnown::new(self.request.clone());
                    self.resume(None)
                }
                None => DiscoveryCoroutineState::Complete(Ok(Some(DiscoveryHttpWalkOutput {
                    url: self.request.url.clone(),
                    hops: self.hops,
                    response: output.response,
                }))),
            },
            HttpCoroutineState::Complete(Err(err)) => DiscoveryCoroutineState::Complete(Err(err)),
        }
    }
}
