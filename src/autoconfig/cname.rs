//! # DNS CNAME query coroutine
//!
//! [`DiscoveryDnsCname`] sends one DNS A question through the shared
//! [`DiscoveryDnsExchange`] transport, whose answer carries the whole
//! CNAME chain (a CNAME question gets its first hop only), and follows
//! that chain to its last target.
//!
//! A provider serving the autoconfig of the domains it hosts has them
//! alias `autoconfig.<domain>` to its own host, whose TLS certificate
//! rarely covers the alias: fetching the target instead authenticates
//! the provider. The alias is trusted as much as the DNS answer
//! carrying it, the trust an MX record already gets.

use core::mem;

use alloc::{
    string::{String, ToString},
    vec::Vec,
};

use domain::new::{
    base::{
        HeaderFlags, MessageItem, QClass, QType, Question,
        build::{MessageBuildError, MessageBuilder},
        name::{NameCompressor, NameParseError, RevNameBuf},
        parse::MessageParser,
        wire::{AsBytes, U16},
    },
    rdata::RecordData,
};
use thiserror::Error;
use url::Url;

use crate::{
    coroutine::{DiscoveryCoroutine, DiscoveryCoroutineState, DiscoveryYield},
    shared::dns::{
        DNS_QUERY_BUF_SIZE, DiscoveryDnsExchange, DiscoveryDnsExchangeError, absolute_name,
    },
};

/// Errors that can occur during a single DNS CNAME exchange.
#[derive(Debug, Error)]
pub enum DiscoveryDnsCnameError {
    /// The queried domain is not a valid DNS name.
    #[error("DNS CNAME domain `{1}` is not a valid name")]
    InvalidDomain(#[source] NameParseError, String),
    /// The built query did not fit in the fixed query buffer.
    #[error("DNS CNAME query did not fit in the {DNS_QUERY_BUF_SIZE}-byte buffer")]
    QueryTooLarge(#[source] MessageBuildError),
    /// The CNAME response could not be parsed.
    #[error("DNS CNAME response could not be parsed")]
    InvalidResponse(String),
    /// The stream reached EOF before a full response arrived.
    #[error("DNS CNAME stream reached EOF before a full response")]
    Eof,
    /// The underlying DNS message exchange failed.
    #[error("DNS CNAME exchange failed")]
    Exchange(#[source] DiscoveryDnsExchangeError),
}

/// Internal state of the [`DiscoveryDnsCname`] coroutine.
#[derive(Debug, Default)]
enum State {
    /// First step: the coroutine still has to build the query message.
    BuildQuery,
    /// The query is travelling to the resolver and back.
    Exchange(DiscoveryDnsExchange),
    /// `Complete` has already been returned.
    #[default]
    Done,
}

/// I/O-free coroutine resolving the name a domain aliases.
#[derive(Debug)]
pub struct DiscoveryDnsCname {
    domain: String,
    resolver: Url,
    state: State,
}

impl DiscoveryDnsCname {
    /// Returns a coroutine ready to build and emit a DNS A query for
    /// `domain` on the first [`resume`]. `resolver` is a
    /// `tcp://host:port` DNS-over-TCP resolver or an RFC 8484
    /// `https://…/dns-query` one.
    ///
    /// [`resume`]: DiscoveryDnsCname::resume
    pub fn new(domain: impl ToString, resolver: Url) -> Self {
        Self {
            domain: domain.to_string(),
            resolver,
            state: State::BuildQuery,
        }
    }
}

impl DiscoveryCoroutine for DiscoveryDnsCname {
    type Yield = DiscoveryYield;
    type Return = Result<Option<String>, DiscoveryDnsCnameError>;

    fn resume(&mut self, arg: Option<&[u8]>) -> DiscoveryCoroutineState<Self::Yield, Self::Return> {
        match mem::take(&mut self.state) {
            State::BuildQuery => {
                let qname = match absolute_name(&self.domain).parse::<RevNameBuf>() {
                    Ok(qname) => qname,
                    Err(err) => {
                        let domain = mem::take(&mut self.domain);
                        return DiscoveryCoroutineState::Complete(Err(
                            DiscoveryDnsCnameError::InvalidDomain(err, domain),
                        ));
                    }
                };

                let mut buf = vec![0u8; DNS_QUERY_BUF_SIZE];
                let mut compressor = NameCompressor::default();
                let mut builder = MessageBuilder::new(
                    &mut buf,
                    &mut compressor,
                    U16::new(1),
                    *HeaderFlags::default().set_rd(true),
                );

                let q = Question {
                    qname,
                    qtype: QType::A,
                    qclass: QClass::IN,
                };

                if let Err(err) = builder.push_question(&q) {
                    return DiscoveryCoroutineState::Complete(Err(
                        DiscoveryDnsCnameError::QueryTooLarge(err),
                    ));
                }

                let message = builder.finish().as_bytes().to_vec();
                let exchange = DiscoveryDnsExchange::new(message, self.resolver.clone());

                self.state = State::Exchange(exchange);
                self.resume(None)
            }

            State::Exchange(mut exchange) => match exchange.resume(arg) {
                DiscoveryCoroutineState::Yielded(y) => {
                    self.state = State::Exchange(exchange);
                    DiscoveryCoroutineState::Yielded(y)
                }
                DiscoveryCoroutineState::Complete(Err(DiscoveryDnsExchangeError::Eof)) => {
                    DiscoveryCoroutineState::Complete(Err(DiscoveryDnsCnameError::Eof))
                }
                DiscoveryCoroutineState::Complete(Err(err)) => {
                    DiscoveryCoroutineState::Complete(Err(DiscoveryDnsCnameError::Exchange(err)))
                }
                DiscoveryCoroutineState::Complete(Ok(response)) => {
                    let parser = match MessageParser::new(&response) {
                        Ok(parser) => parser,
                        Err(err) => {
                            return DiscoveryCoroutineState::Complete(Err(
                                DiscoveryDnsCnameError::InvalidResponse(err.to_string()),
                            ));
                        }
                    };

                    let mut aliases: Vec<(String, String)> = Vec::new();

                    for item in parser {
                        let Ok(MessageItem::Answer(record)) = item else {
                            continue;
                        };

                        let RecordData::CName(cname) = record.rdata else {
                            continue;
                        };

                        aliases.push((
                            normalize(&record.rname.to_name().to_string()),
                            normalize(&cname.name.to_string()),
                        ));
                    }

                    DiscoveryCoroutineState::Complete(Ok(follow(&self.domain, &aliases)))
                }
            },

            State::Done => {
                panic!("DiscoveryDnsCname::resume called after completion")
            }
        }
    }
}

/// Lowercases a DNS name and strips its trailing dot.
fn normalize(name: &str) -> String {
    name.trim_end_matches('.').to_ascii_lowercase()
}

/// Follows the `(owner, target)` aliases from `domain` to the last
/// target. Bounded by the alias count, so a loop ends.
fn follow(domain: &str, aliases: &[(String, String)]) -> Option<String> {
    let mut current = normalize(domain);
    let mut target = None;

    for _ in 0..aliases.len() {
        let Some((_, next)) = aliases.iter().find(|(owner, _)| *owner == current) else {
            break;
        };
        current = next.clone();
        target = Some(next.clone());
    }

    target
}

#[cfg(test)]
mod tests {
    use crate::autoconfig::cname::*;

    #[test]
    fn follow_reaches_the_last_target_of_a_chain() {
        let aliases = vec![
            ("b.example.org".to_string(), "c.example.net".to_string()),
            ("a.example.com".to_string(), "b.example.org".to_string()),
        ];
        assert_eq!(
            follow("A.example.com.", &aliases),
            Some("c.example.net".to_string())
        );
        assert_eq!(follow("other.example.com", &aliases), None);

        let looped = vec![
            ("a.example.com".to_string(), "b.example.com".to_string()),
            ("b.example.com".to_string(), "a.example.com".to_string()),
        ];
        assert!(follow("a.example.com", &looped).is_some());
    }
}
