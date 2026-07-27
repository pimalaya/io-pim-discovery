//! # Standard, blocking compose client
//!
//! [`DiscoveryComposeClientStd`] orchestrates the discovery bricks in parallel:
//! one OS thread per mechanism, each pumping its coroutine through
//! its own [`DiscoveryStreamPool`], the outputs reduced in mechanism-priority
//! order by the pure [`DiscoveryConfigCollector`]. A final probe pass then
//! asks each collected HTTP endpoint which authentication schemes it
//! advertises on its unauthenticated 401 (PACC §5.4.2) and refines
//! the config's password and bearer methods accordingly, one thread
//! per config.
//!
//! Mechanism failures are logged and skipped: only an invalid email
//! address fails the whole compose. Mechanisms irrelevant to the
//! requested services are never started.
//!
//! [`compose_all`](DiscoveryComposeClientStd::compose_all) waits for
//! every mechanism to finish. [`compose_all_within`](DiscoveryComposeClientStd::compose_all_within)
//! bounds that wait to a deadline for interactive callers: each
//! mechanism runs on its own detached thread and any still running at
//! the deadline is abandoned, so a single unreachable endpoint cannot
//! stall the whole discovery.

use std::{
    sync::{Arc, mpsc},
    thread,
    time::{Duration, Instant},
};

#[cfg(feature = "rfc8414")]
use alloc::collections::BTreeMap;
use alloc::{
    collections::BTreeSet,
    string::{String, ToString},
    vec::Vec,
};

use log::{debug, trace};
use pimalaya_stream::tls::Tls;
use thiserror::Error;
use url::Url;

#[cfg(feature = "autoconfig")]
use crate::autoconfig::{isp::DiscoveryIsp, mailconf::DiscoveryMailconf, mx::DiscoveryDnsMx};
#[cfg(feature = "rfc8414")]
use crate::compose::config::DiscoveryAuthMethod;
#[cfg(feature = "autoconfig")]
use crate::compose::config::DiscoveryConfigSource;
#[cfg(feature = "pacc")]
use crate::pacc::discover::DiscoveryPacc;
#[cfg(feature = "rfc6186")]
use crate::rfc6186::discover::DiscoverySrv;
#[cfg(feature = "rfc6764")]
use crate::rfc6764::{resolve::DiscoveryDavResolve, service::DiscoveryDavService};
#[cfg(feature = "rfc8414")]
use crate::rfc8414::{DiscoveryOauthServerMetadata, DiscoveryOauthServerResolve};
#[cfg(feature = "rfc8620")]
use crate::rfc8620::resolve::DiscoveryJmapResolve;
#[cfg(feature = "rfc8620")]
use crate::rfc9110::DiscoveryProbeAuth;
#[cfg(feature = "rfc9728")]
use crate::rfc9728::{DiscoveryOauthResourceMetadata, DiscoveryOauthResourceResolve};
use crate::{
    compose::{
        collect::DiscoveryConfigCollector,
        config::{DiscoveryService, DiscoveryServiceConfig},
        providers::DiscoveryKnownProvider,
    },
    coroutine::{DiscoveryCoroutine, DiscoveryCoroutineState, DiscoveryYield},
    shared::pool::DiscoveryStreamPool,
};

const READ_BUFFER_SIZE: usize = 8 * 1024;

/// Errors returned by [`DiscoveryComposeClientStd`].
#[derive(Debug, Error)]
pub enum DiscoveryComposeClientStdError {
    /// The input is not a valid `local@domain` email address.
    #[error("Email address `{0}` is missing the `@` separator")]
    InvalidEmail(String),
}

/// Std-blocking parallel compose orchestrator.
pub struct DiscoveryComposeClientStd {
    dns: Url,
    tls: Tls,
}

/// A single discovery mechanism the fan-out can run, dispatched by
/// [`DiscoveryComposeClientStd::run_mechanism`]. Kept in one place so
/// the bounded and unbounded fan-outs share the exact same priority
/// order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mechanism {
    Mx,
    Pacc,
    IspMain,
    IspFallback,
    Mailconf,
    Ispdb,
    Srv,
    #[cfg(feature = "rfc6764")]
    Caldav,
    #[cfg(feature = "rfc6764")]
    Carddav,
    Jmap,
}

/// The outcome of [`DiscoveryComposeClientStd::plan`]: the pure
/// fixed-provider output (always reduced first, `None` when no rule
/// matched) plus the parsed inputs and the ordered mechanisms to run.
struct Plan {
    provider_output: Option<Vec<DiscoveryServiceConfig>>,
    local: String,
    domain: String,
    mechanisms: Vec<Mechanism>,
}

impl DiscoveryComposeClientStd {
    /// Builds a client that resolves DNS lookups through `dns` (a
    /// `tcp://host:port` URL pointing at a DNS-over-TCP resolver) and
    /// runs the HTTPS-bound mechanisms over `tls`.
    pub fn new(dns: Url, tls: Tls) -> Self {
        Self { dns, tls }
    }

    /// Runs every mechanism in parallel and returns all configs found
    /// for `email`, restricted to `services` (empty means all
    /// services).
    pub fn compose_all(
        &self,
        email: &str,
        services: BTreeSet<DiscoveryService>,
    ) -> Result<Vec<DiscoveryServiceConfig>, DiscoveryComposeClientStdError> {
        self.compose(email, services, false, None)
    }

    /// Same as [`compose_all`](Self::compose_all), but bounds the
    /// parallel mechanism discovery to `deadline`: mechanisms that have
    /// not produced their configs by then are abandoned (they finish in
    /// the background and their output is dropped), and only what
    /// completed in time is reduced and returned. Meant for interactive
    /// callers (a setup wizard) where a single unreachable endpoint must
    /// not stall the whole discovery. An empty result means nothing
    /// completed in time.
    pub fn compose_all_within(
        &self,
        email: &str,
        services: BTreeSet<DiscoveryService>,
        deadline: Duration,
    ) -> Result<Vec<DiscoveryServiceConfig>, DiscoveryComposeClientStdError> {
        self.compose(email, services, false, Some(deadline))
    }

    /// Same mechanism set as [`compose_all`](Self::compose_all), but
    /// keeps only the configs of the highest-priority mechanism that
    /// produced any; an empty result means no mechanism produced
    /// anything. The mechanisms still run in parallel, so this trades
    /// no latency, only output size.
    pub fn compose_first(
        &self,
        email: &str,
        services: BTreeSet<DiscoveryService>,
    ) -> Result<Vec<DiscoveryServiceConfig>, DiscoveryComposeClientStdError> {
        self.compose(email, services, true, None)
    }

    /// Runs every mechanism in parallel and returns their raw,
    /// unmerged configs for `email`, restricted to `services` (empty
    /// means all). Unlike [`compose_all`](Self::compose_all), the
    /// per-mechanism outputs are not reduced against each other: each
    /// config keeps its own source and cross-mechanism duplicates are
    /// preserved.
    pub fn compose_raw(
        &self,
        email: &str,
        services: BTreeSet<DiscoveryService>,
    ) -> Result<Vec<DiscoveryServiceConfig>, DiscoveryComposeClientStdError> {
        let outputs = self.parallel_outputs(email, &services)?;

        let mut configs: Vec<DiscoveryServiceConfig> = outputs
            .into_iter()
            .flatten()
            .filter(|config| services.is_empty() || services.contains(&config.service))
            .collect();

        self.resolve_issuers(&mut configs);

        Ok(configs)
    }

    /// Discovers the fixed-provider configs for `email`: the domain
    /// rule first, then MX-based detection. Raw and unmerged.
    pub fn provider(&self, email: &str) -> Vec<DiscoveryServiceConfig> {
        self.detect_provider(email)
            .map(|provider| provider.configs(email))
            .unwrap_or_default()
    }

    /// The fixed Google configs for `email` when it is Google-hosted
    /// (domain rule or MX records), otherwise empty.
    pub fn is_google(&self, email: &str) -> Vec<DiscoveryServiceConfig> {
        match self.detect_provider(email) {
            Some(DiscoveryKnownProvider::Google) => DiscoveryKnownProvider::Google.configs(email),
            _ => Vec::new(),
        }
    }

    /// The fixed Microsoft configs for `email` when it is
    /// Microsoft-hosted (domain rule or MX records), otherwise empty.
    pub fn is_microsoft(&self, email: &str) -> Vec<DiscoveryServiceConfig> {
        match self.detect_provider(email) {
            Some(DiscoveryKnownProvider::Microsoft) => {
                DiscoveryKnownProvider::Microsoft.configs(email)
            }
            _ => Vec::new(),
        }
    }

    /// Runs every Mozilla autoconfig location (ISP main, ISP fallback,
    /// ISPDB, mailconf) for `email`. Raw and unmerged.
    #[cfg(feature = "autoconfig")]
    pub fn autoconfig(&self, email: &str) -> Vec<DiscoveryServiceConfig> {
        let local = email.split_once('@').map(|(local, _)| local).unwrap_or("");
        let domain = domain_part(email);

        let mut configs = Vec::new();
        configs.extend(self.run_isp_main(local, &domain, email));
        configs.extend(self.run_isp_fallback(&domain, email));
        configs.extend(self.run_ispdb(&domain, email));
        configs.extend(self.run_mailconf(&domain, email));
        configs
    }

    /// Runs RFC 6186 SRV mail discovery for `input` (an email address
    /// or a bare domain). Raw.
    #[cfg(feature = "rfc6186")]
    pub fn srv(&self, input: &str) -> Vec<DiscoveryServiceConfig> {
        self.run_srv(&domain_part(input))
    }

    /// Runs PACC discovery for `input` (an email address or a bare
    /// domain). Raw.
    #[cfg(feature = "pacc")]
    pub fn pacc(&self, input: &str) -> Vec<DiscoveryServiceConfig> {
        self.run_pacc(&domain_part(input))
    }

    /// Runs RFC 6764 CalDAV or CardDAV resolution for `input` (an
    /// email address or a bare domain). Raw.
    #[cfg(feature = "rfc6764")]
    pub fn dav(&self, input: &str, service: DiscoveryDavService) -> Vec<DiscoveryServiceConfig> {
        self.run_dav(&domain_part(input), service)
    }

    /// Runs RFC 8620 JMAP session resolution for `input` (an email
    /// address or a bare domain). Raw.
    #[cfg(feature = "rfc8620")]
    pub fn jmap(&self, input: &str) -> Vec<DiscoveryServiceConfig> {
        self.run_jmap(&domain_part(input))
    }

    /// Probes `url` for the authentication schemes it advertises on an
    /// unauthenticated 401 response. `None` when the probe failed or
    /// nothing was advertised.
    #[cfg(feature = "rfc8620")]
    pub fn auth(&self, url: Url) -> Option<Vec<String>> {
        match run(&mut self.pool(), DiscoveryProbeAuth::new(url)) {
            Ok(schemes) if !schemes.is_empty() => Some(schemes),
            _ => None,
        }
    }

    /// Fetches `issuer`'s RFC 8414 authorization server metadata,
    /// trying the OAuth well-known URL then the OpenID Connect
    /// Discovery one. `None` when neither resolves.
    #[cfg(feature = "rfc8414")]
    pub fn oauth_server(&self, issuer: &Url) -> Option<DiscoveryOauthServerMetadata> {
        let well_known = DiscoveryOauthServerMetadata::well_known_url(issuer);
        if let Ok(metadata) = run(
            &mut self.pool(),
            DiscoveryOauthServerResolve::new(well_known),
        ) {
            return Some(metadata);
        }

        let openid = DiscoveryOauthServerMetadata::openid_well_known_url(issuer);
        run(&mut self.pool(), DiscoveryOauthServerResolve::new(openid)).ok()
    }

    /// Fetches `resource`'s RFC 9728 protected resource metadata from
    /// its well-known URL. `None` when it does not resolve.
    #[cfg(feature = "rfc9728")]
    pub fn oauth_resource(&self, resource: &Url) -> Option<DiscoveryOauthResourceMetadata> {
        let well_known = DiscoveryOauthResourceMetadata::well_known_url(resource);
        run(
            &mut self.pool(),
            DiscoveryOauthResourceResolve::new(well_known),
        )
        .ok()
    }

    fn compose(
        &self,
        email: &str,
        services: BTreeSet<DiscoveryService>,
        first: bool,
        deadline: Option<Duration>,
    ) -> Result<Vec<DiscoveryServiceConfig>, DiscoveryComposeClientStdError> {
        debug!("begin config compose");
        trace!("email {email}, first: {first}, services: {services:?}, deadline: {deadline:?}");

        let outputs = match deadline {
            Some(deadline) => self.parallel_outputs_within(email, &services, deadline)?,
            None => self.parallel_outputs(email, &services)?,
        };
        let mut collector = DiscoveryConfigCollector::new(services);

        for configs in outputs {
            collector.collect(configs);

            if first && !collector.is_empty() {
                debug!("keep first mechanism yielding configs");
                break;
            }
        }

        let mut configs = collector.finish();
        self.probe(&mut configs);
        self.resolve_issuers(&mut configs);

        debug!("end of config compose");
        trace!("{configs:?}");
        Ok(configs)
    }

    /// Runs every mechanism relevant to `services` in parallel (one
    /// thread each) and returns their outputs in mechanism-priority
    /// order, one entry per mechanism, unreduced.
    fn parallel_outputs(
        &self,
        email: &str,
        services: &BTreeSet<DiscoveryService>,
    ) -> Result<Vec<Vec<DiscoveryServiceConfig>>, DiscoveryComposeClientStdError> {
        let Plan {
            provider_output,
            local,
            domain,
            mechanisms,
        } = self.plan(email, services)?;

        // Mechanism outputs, in priority order. The fixed provider
        // domain rule is pure and comes first (see `plan`).
        let mut outputs: Vec<Vec<DiscoveryServiceConfig>> = Vec::new();
        outputs.extend(provider_output);

        outputs.extend(thread::scope(|scope| {
            let (local, domain, email) = (local.as_str(), domain.as_str(), email.trim());
            let handles: Vec<_> = mechanisms
                .iter()
                .map(|&mechanism| {
                    scope.spawn(move || self.run_mechanism(mechanism, local, domain, email))
                })
                .collect();

            handles
                .into_iter()
                .map(|handle| handle.join().unwrap_or_default())
                .collect::<Vec<_>>()
        }));

        Ok(outputs)
    }

    /// Same as [`parallel_outputs`](Self::parallel_outputs) but bounds
    /// the mechanism fan-out to `deadline`. Each mechanism runs on its
    /// own detached thread; those that have not reported by the deadline
    /// leave an empty slot (their thread finishes in the background and
    /// its output is dropped), so the returned vec keeps the same
    /// mechanism-priority order regardless of completion order.
    fn parallel_outputs_within(
        &self,
        email: &str,
        services: &BTreeSet<DiscoveryService>,
        deadline: Duration,
    ) -> Result<Vec<Vec<DiscoveryServiceConfig>>, DiscoveryComposeClientStdError> {
        let Plan {
            provider_output,
            local,
            domain,
            mechanisms,
        } = self.plan(email, services)?;

        let mut outputs: Vec<Vec<DiscoveryServiceConfig>> = Vec::new();
        outputs.extend(provider_output);

        // NOTE: detached threads (unlike a scope) may outlive this call,
        // so each captures an owned, refcounted client and owned inputs
        // rather than borrowing `self`. Stragglers past the deadline run
        // to completion in the background against their own clone.
        let client = Arc::new(self.clone_shallow());
        let email = email.trim().to_string();
        let tasks: Vec<_> = mechanisms
            .into_iter()
            .map(|mechanism| {
                let client = client.clone();
                let (local, domain, email) = (local.clone(), domain.clone(), email.clone());
                move || client.run_mechanism(mechanism, &local, &domain, &email)
            })
            .collect();

        for output in collect_within(tasks, deadline) {
            outputs.push(output.unwrap_or_default());
        }

        Ok(outputs)
    }

    /// Parses `email` and works out which mechanisms to run for
    /// `services`, in priority order, shared by the bounded and
    /// unbounded fan-outs. The fixed-provider domain rule is pure, so it
    /// is resolved here and returned as the always-first output; when it
    /// matches, the MX-based provider detection is pointless and skipped.
    fn plan(
        &self,
        email: &str,
        services: &BTreeSet<DiscoveryService>,
    ) -> Result<Plan, DiscoveryComposeClientStdError> {
        let email = email.trim();

        let Some((local, domain)) = email.split_once('@') else {
            return Err(DiscoveryComposeClientStdError::InvalidEmail(
                email.to_string(),
            ));
        };
        let domain = domain.trim_matches('.').to_ascii_lowercase();

        let wants = |service: DiscoveryService| services.is_empty() || services.contains(&service);
        let wants_mail = wants(DiscoveryService::Imap)
            || wants(DiscoveryService::Pop3)
            || wants(DiscoveryService::Smtp);

        let provider = DiscoveryKnownProvider::from_domain(&domain);
        let provider_output = provider.map(|provider| {
            debug!("email domain matched a fixed provider rule");
            trace!("{domain} -> {provider:?}");
            provider.configs(email)
        });

        // Mechanisms in priority order, matching the reduction order the
        // collector relies on.
        let mut mechanisms = Vec::new();
        if provider.is_none() {
            mechanisms.push(Mechanism::Mx);
        }
        mechanisms.push(Mechanism::Pacc);
        if wants_mail {
            mechanisms.extend([
                Mechanism::IspMain,
                Mechanism::IspFallback,
                Mechanism::Mailconf,
                Mechanism::Ispdb,
            ]);
        }
        if wants(DiscoveryService::Imap) || wants(DiscoveryService::Smtp) {
            mechanisms.push(Mechanism::Srv);
        }
        #[cfg(feature = "rfc6764")]
        if wants(DiscoveryService::Caldav) {
            mechanisms.push(Mechanism::Caldav);
        }
        #[cfg(feature = "rfc6764")]
        if wants(DiscoveryService::Carddav) {
            mechanisms.push(Mechanism::Carddav);
        }
        if wants(DiscoveryService::Jmap) {
            mechanisms.push(Mechanism::Jmap);
        }

        Ok(Plan {
            provider_output,
            local: local.to_string(),
            domain,
            mechanisms,
        })
    }

    /// Dispatches one [`Mechanism`] to its runner. The runners are pure
    /// with respect to `self` (they read only the DNS resolver and TLS
    /// config), so this is safe to call from both scoped and detached
    /// threads.
    fn run_mechanism(
        &self,
        mechanism: Mechanism,
        local: &str,
        domain: &str,
        email: &str,
    ) -> Vec<DiscoveryServiceConfig> {
        match mechanism {
            Mechanism::Mx => self.run_mx(domain, email),
            Mechanism::Pacc => self.run_pacc(domain),
            Mechanism::IspMain => self.run_isp_main(local, domain, email),
            Mechanism::IspFallback => self.run_isp_fallback(domain, email),
            Mechanism::Mailconf => self.run_mailconf(domain, email),
            Mechanism::Ispdb => self.run_ispdb(domain, email),
            Mechanism::Srv => self.run_srv(domain),
            #[cfg(feature = "rfc6764")]
            Mechanism::Caldav => self.run_dav(domain, DiscoveryDavService::Caldav),
            #[cfg(feature = "rfc6764")]
            Mechanism::Carddav => self.run_dav(domain, DiscoveryDavService::Carddav),
            Mechanism::Jmap => self.run_jmap(domain),
        }
    }

    /// Cheap copy carrying only the fields the mechanism runners read,
    /// so a detached thread can own the client instead of borrowing it.
    fn clone_shallow(&self) -> Self {
        Self {
            dns: self.dns.clone(),
            tls: self.tls.clone(),
        }
    }

    /// Resolves every `OauthIssuer` auth method in place: fetches the
    /// issuer's RFC 8414 metadata and replaces the bare issuer with
    /// the concrete grants it advertises. Each distinct issuer is
    /// resolved once, in parallel; unresolvable issuers are left as
    /// they were.
    #[cfg(feature = "rfc8414")]
    fn resolve_issuers(&self, configs: &mut [DiscoveryServiceConfig]) {
        let issuers: BTreeSet<String> = configs
            .iter()
            .flat_map(|config| &config.auth)
            .filter_map(|method| match method {
                DiscoveryAuthMethod::OauthIssuer(issuer) => Some(issuer.clone()),
                _ => None,
            })
            .collect();

        if issuers.is_empty() {
            return;
        }

        let resolved: BTreeMap<String, Vec<DiscoveryAuthMethod>> = thread::scope(|scope| {
            let handles: Vec<_> = issuers
                .iter()
                .map(|issuer| scope.spawn(move || (issuer.clone(), self.resolve_issuer(issuer))))
                .collect();

            handles
                .into_iter()
                .filter_map(|handle| handle.join().ok())
                .collect()
        });

        for config in configs.iter_mut() {
            let mut auth = Vec::new();

            for method in config.auth.drain(..) {
                match method {
                    DiscoveryAuthMethod::OauthIssuer(issuer) => match resolved.get(&issuer) {
                        Some(methods) => auth.extend(methods.iter().cloned()),
                        None => auth.push(DiscoveryAuthMethod::OauthIssuer(issuer)),
                    },
                    other => auth.push(other),
                }
            }

            config.auth = auth;
        }
    }

    /// No-op when RFC 8414 is not compiled in: discovered issuers stay
    /// as bare `OauthIssuer` methods.
    #[cfg(not(feature = "rfc8414"))]
    fn resolve_issuers(&self, _configs: &mut [DiscoveryServiceConfig]) {}

    /// Resolves one issuer to the grants its RFC 8414 metadata
    /// advertises (authorization code grant, plus device grant when
    /// the metadata names a device authorization endpoint). Falls back
    /// to the bare issuer when the metadata cannot be fetched or names
    /// no usable endpoints.
    #[cfg(feature = "rfc8414")]
    fn resolve_issuer(&self, issuer: &str) -> Vec<DiscoveryAuthMethod> {
        let bare = || vec![DiscoveryAuthMethod::OauthIssuer(issuer.to_string())];

        let Ok(issuer_url) = Url::parse(issuer) else {
            return bare();
        };
        let Some(metadata) = self.oauth_server(&issuer_url) else {
            debug!("skip unresolvable OAuth issuer");
            trace!("{issuer}");
            return bare();
        };

        let mut methods = Vec::new();

        if let (Some(authorization), Some(token)) =
            (&metadata.authorization_endpoint, &metadata.token_endpoint)
        {
            methods.push(DiscoveryAuthMethod::OauthAuthorizationCodeGrant {
                authorization_endpoint: authorization.to_string(),
                token_endpoint: token.to_string(),
                scope: None,
            });
        }

        if let (Some(device), Some(token)) = (
            &metadata.device_authorization_endpoint,
            &metadata.token_endpoint,
        ) {
            methods.push(DiscoveryAuthMethod::OauthDeviceAuthorizationGrant {
                device_authorization_endpoint: device.to_string(),
                token_endpoint: token.to_string(),
                scope: None,
            });
        }

        if methods.is_empty() { bare() } else { methods }
    }

    /// A fresh stream pool for one mechanism thread: the default
    /// `tcp` factory for DNS lookups, plus `http`/`https` factories
    /// backed by the client's TLS.
    fn pool(&self) -> DiscoveryStreamPool {
        DiscoveryStreamPool::new().with_http_factories(self.tls.clone())
    }

    /// Probes each config's endpoints for their advertised
    /// authentication schemes, in parallel, and refines the configs
    /// in place. Within one config, the URLs are tried in order until
    /// one advertises any scheme.
    #[cfg(feature = "rfc8620")]
    fn probe(&self, configs: &mut [DiscoveryServiceConfig]) {
        let schemes: Vec<Option<Vec<String>>> = thread::scope(|scope| {
            let handles: Vec<_> = configs
                .iter()
                .map(|config| {
                    let urls = config.probe_urls();
                    scope.spawn(move || {
                        for url in urls {
                            debug!("probe endpoint authentication schemes");
                            trace!("{url}");

                            match run(&mut self.pool(), DiscoveryProbeAuth::new(url)) {
                                Ok(schemes) if !schemes.is_empty() => return Some(schemes),
                                // Nothing learned at this URL: the
                                // config's next URL gets its turn.
                                Ok(_) => {}
                                Err(err) => {
                                    debug!("skip failed auth probe");
                                    trace!("{err:?}");
                                }
                            }
                        }
                        None
                    })
                })
                .collect();

            handles
                .into_iter()
                .map(|handle| handle.join().unwrap_or(None))
                .collect()
        });

        for (config, schemes) in configs.iter_mut().zip(schemes) {
            if let Some(schemes) = schemes {
                config.refine_auth(&schemes);
            }
        }
    }

    /// No-op when the auth probe (RFC 9110, behind `rfc8620`) is not
    /// compiled in: configs keep the auth methods their mechanism
    /// reported.
    #[cfg(not(feature = "rfc8620"))]
    fn probe(&self, _configs: &mut [DiscoveryServiceConfig]) {}

    /// Detects the fixed provider hosting `email`: the domain rule
    /// first, then MX-based detection. `None` when neither matches.
    fn detect_provider(&self, email: &str) -> Option<DiscoveryKnownProvider> {
        let domain = domain_part(email);
        DiscoveryKnownProvider::from_domain(&domain).or_else(|| self.provider_from_mx(&domain))
    }

    /// Looks up `domain`'s MX records and returns the first fixed
    /// provider (Google Workspace, Microsoft 365) they match.
    #[cfg(feature = "autoconfig")]
    fn provider_from_mx(&self, domain: &str) -> Option<DiscoveryKnownProvider> {
        let mx = DiscoveryDnsMx::new(domain, self.dns.clone());

        let records = match run(&mut self.pool(), mx) {
            Ok(records) => records,
            Err(err) => {
                debug!("skip MX provider detection");
                trace!("{err:?}");
                return None;
            }
        };

        for record in records {
            let exchange = record.rdata.exchange.to_string();

            if let Some(provider) = DiscoveryKnownProvider::from_mx(&exchange) {
                debug!("MX record matched a fixed provider rule");
                trace!("{exchange} -> {provider:?}");
                return Some(provider);
            }
        }

        None
    }

    /// The fixed configs of the provider hosting `domain` per its MX
    /// records, or empty.
    #[cfg(feature = "autoconfig")]
    fn run_mx(&self, domain: &str, email: &str) -> Vec<DiscoveryServiceConfig> {
        self.provider_from_mx(domain)
            .map(|provider| provider.configs(email))
            .unwrap_or_default()
    }

    /// No-op stubs when `autoconfig` (which owns the MX coroutine) is
    /// off: provider detection falls back to the pure domain rule.
    #[cfg(not(feature = "autoconfig"))]
    fn provider_from_mx(&self, _domain: &str) -> Option<DiscoveryKnownProvider> {
        None
    }

    #[cfg(not(feature = "autoconfig"))]
    fn run_mx(&self, _domain: &str, _email: &str) -> Vec<DiscoveryServiceConfig> {
        Vec::new()
    }

    #[cfg(feature = "pacc")]
    fn run_pacc(&self, domain: &str) -> Vec<DiscoveryServiceConfig> {
        let pacc = match DiscoveryPacc::new(domain, self.dns.clone()) {
            Ok(pacc) => pacc,
            Err(err) => {
                debug!("skip PACC discovery");
                trace!("{err:?}");
                return Vec::new();
            }
        };

        match run(&mut self.pool(), pacc) {
            Ok(config) => DiscoveryServiceConfig::from_pacc(&config),
            Err(err) => {
                debug!("skip PACC discovery");
                trace!("{err:?}");
                Vec::new()
            }
        }
    }

    #[cfg(not(feature = "pacc"))]
    fn run_pacc(&self, _domain: &str) -> Vec<DiscoveryServiceConfig> {
        Vec::new()
    }

    #[cfg(feature = "autoconfig")]
    fn run_isp_main(&self, local: &str, domain: &str, email: &str) -> Vec<DiscoveryServiceConfig> {
        match DiscoveryIsp::main_url(local, domain, true) {
            Ok(url) => self.run_isp(url, email, DiscoveryConfigSource::IspMain),
            Err(err) => {
                debug!("skip autoconfig ISP main URL");
                trace!("{err:?}");
                Vec::new()
            }
        }
    }

    #[cfg(feature = "autoconfig")]
    fn run_isp_fallback(&self, domain: &str, email: &str) -> Vec<DiscoveryServiceConfig> {
        match DiscoveryIsp::fallback_url(domain, true) {
            Ok(url) => self.run_isp(url, email, DiscoveryConfigSource::IspFallback),
            Err(err) => {
                debug!("skip autoconfig ISP fallback URL");
                trace!("{err:?}");
                Vec::new()
            }
        }
    }

    #[cfg(feature = "autoconfig")]
    fn run_ispdb(&self, domain: &str, email: &str) -> Vec<DiscoveryServiceConfig> {
        match DiscoveryIsp::db_url(domain, true) {
            Ok(url) => self.run_isp(url, email, DiscoveryConfigSource::Ispdb),
            Err(err) => {
                debug!("skip autoconfig ISPDB URL");
                trace!("{err:?}");
                Vec::new()
            }
        }
    }

    /// Follows the mailconf TXT redirect to its autoconfig document.
    #[cfg(feature = "autoconfig")]
    fn run_mailconf(&self, domain: &str, email: &str) -> Vec<DiscoveryServiceConfig> {
        let mailconf = DiscoveryMailconf::new(domain, self.dns.clone());

        match run(&mut self.pool(), mailconf) {
            Ok(url) => {
                debug!("follow mailconf TXT redirect");
                trace!("{url}");
                self.run_isp(url, email, DiscoveryConfigSource::Mailconf)
            }
            Err(err) => {
                debug!("skip mailconf TXT redirect");
                trace!("{err:?}");
                Vec::new()
            }
        }
    }

    #[cfg(feature = "autoconfig")]
    fn run_isp(
        &self,
        url: Url,
        email: &str,
        source: DiscoveryConfigSource,
    ) -> Vec<DiscoveryServiceConfig> {
        match run(&mut self.pool(), DiscoveryIsp::new(url)) {
            Ok(config) => DiscoveryServiceConfig::from_autoconfig(&config, email, source),
            Err(err) => {
                debug!("skip autoconfig document");
                trace!("{err:?}");
                Vec::new()
            }
        }
    }

    /// No-op autoconfig stubs when `autoconfig` is off, so the
    /// orchestrator calls them without a cfg at each site.
    #[cfg(not(feature = "autoconfig"))]
    fn run_isp_main(
        &self,
        _local: &str,
        _domain: &str,
        _email: &str,
    ) -> Vec<DiscoveryServiceConfig> {
        Vec::new()
    }

    #[cfg(not(feature = "autoconfig"))]
    fn run_isp_fallback(&self, _domain: &str, _email: &str) -> Vec<DiscoveryServiceConfig> {
        Vec::new()
    }

    #[cfg(not(feature = "autoconfig"))]
    fn run_ispdb(&self, _domain: &str, _email: &str) -> Vec<DiscoveryServiceConfig> {
        Vec::new()
    }

    #[cfg(not(feature = "autoconfig"))]
    fn run_mailconf(&self, _domain: &str, _email: &str) -> Vec<DiscoveryServiceConfig> {
        Vec::new()
    }

    #[cfg(feature = "rfc6186")]
    fn run_srv(&self, domain: &str) -> Vec<DiscoveryServiceConfig> {
        let srv = DiscoverySrv::new(domain, self.dns.clone());

        match run(&mut self.pool(), srv) {
            Ok(report) => DiscoveryServiceConfig::from_srv(&report),
            Err(err) => {
                debug!("skip RFC 6186 SRV discovery");
                trace!("{err:?}");
                Vec::new()
            }
        }
    }

    #[cfg(not(feature = "rfc6186"))]
    fn run_srv(&self, _domain: &str) -> Vec<DiscoveryServiceConfig> {
        Vec::new()
    }

    #[cfg(feature = "rfc6764")]
    fn run_dav(&self, domain: &str, service: DiscoveryDavService) -> Vec<DiscoveryServiceConfig> {
        let resolve = DiscoveryDavResolve::new(domain, service, self.dns.clone());

        let config_service = match service {
            DiscoveryDavService::Caldav => DiscoveryService::Caldav,
            DiscoveryDavService::Carddav => DiscoveryService::Carddav,
        };

        match run(&mut self.pool(), resolve) {
            Ok(url) => vec![DiscoveryServiceConfig::from_dav(config_service, url)],
            Err(err) => {
                debug!("skip RFC 6764 DAV resolve");
                trace!("{err:?}");
                Vec::new()
            }
        }
    }

    #[cfg(feature = "rfc8620")]
    fn run_jmap(&self, domain: &str) -> Vec<DiscoveryServiceConfig> {
        let resolve = DiscoveryJmapResolve::new(domain, self.dns.clone());

        match run(&mut self.pool(), resolve) {
            Ok(session) => vec![DiscoveryServiceConfig::from_jmap(
                session.url,
                &session.auth_schemes,
            )],
            Err(err) => {
                debug!("skip RFC 8620 JMAP resolve");
                trace!("{err:?}");
                Vec::new()
            }
        }
    }

    #[cfg(not(feature = "rfc8620"))]
    fn run_jmap(&self, _domain: &str) -> Vec<DiscoveryServiceConfig> {
        Vec::new()
    }
}

/// The lowercased, dot-trimmed domain part of an email address, or the
/// whole input when it carries no `@`.
fn domain_part(email: &str) -> String {
    let domain = email
        .split_once('@')
        .map(|(_, domain)| domain)
        .unwrap_or(email);
    normalize_domain(domain)
}

/// Lowercases and trims surrounding whitespace and dots from a domain.
fn normalize_domain(domain: &str) -> String {
    domain.trim().trim_matches('.').to_ascii_lowercase()
}

/// Pumps one discovery coroutine through the pool until completion.
///
/// I/O failures are not fatal: a stream that cannot be opened, read
/// or written is signalled to the coroutine as EOF (an empty resume
/// slice), so the mechanism errors out on its own and the caller
/// skips it.
fn run<C, T, E>(pool: &mut DiscoveryStreamPool, mut coroutine: C) -> Result<T, E>
where
    C: DiscoveryCoroutine<Yield = DiscoveryYield, Return = Result<T, E>>,
{
    let mut buf = [0u8; READ_BUFFER_SIZE];
    let mut arg: Option<&[u8]> = None;

    loop {
        match coroutine.resume(arg.take()) {
            DiscoveryCoroutineState::Complete(res) => return res,
            DiscoveryCoroutineState::Yielded(DiscoveryYield::WantsRead { url }) => {
                match pool.get(&url).and_then(|s| Ok(s.read(&mut buf)?)) {
                    Ok(n) => arg = Some(&buf[..n]),
                    Err(err) => {
                        debug!("compose read failed, signal EOF");
                        trace!("{url}: {err:?}");
                        arg = Some(&[]);
                    }
                }
            }
            DiscoveryCoroutineState::Yielded(DiscoveryYield::WantsWrite { url, bytes }) => {
                match pool.get(&url).and_then(|s| Ok(s.write_all(&bytes)?)) {
                    Ok(()) => {}
                    Err(err) => {
                        debug!("compose write failed, signal EOF");
                        trace!("{url}: {err:?}");
                        arg = Some(&[]);
                    }
                }
            }
        }
    }
}

/// Runs each task on its own detached thread and collects results by
/// index, waiting no longer than `deadline` in total. A slot stays
/// `None` when its task has not reported by the deadline; that thread
/// keeps running in the background and its result is dropped when it
/// finally sends to the now-disconnected channel. Order matches the
/// input, never completion order.
fn collect_within<T, F>(tasks: Vec<F>, deadline: Duration) -> Vec<Option<T>>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    let count = tasks.len();
    let (tx, rx) = mpsc::channel();

    for (index, task) in tasks.into_iter().enumerate() {
        let tx = tx.clone();
        thread::spawn(move || {
            // The receiver may be gone (deadline passed); dropping the
            // result is the intended straggler behaviour.
            let _ = tx.send((index, task()));
        });
    }
    // Drop the spare sender so `recv` disconnects once every task thread
    // has sent (or panicked), letting the loop finish early when all
    // report before the deadline.
    drop(tx);

    let mut results: Vec<Option<T>> = (0..count).map(|_| None).collect();
    let until = Instant::now() + deadline;

    for _ in 0..count {
        let remaining = until.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        match rx.recv_timeout(remaining) {
            Ok((index, value)) => results[index] = Some(value),
            // Timed out, or every task thread is done: stop waiting.
            Err(_) => break,
        }
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collect_within_preserves_input_order_when_all_finish() {
        let tasks: Vec<fn() -> usize> = vec![|| 0, || 1, || 2];

        let results = collect_within(tasks, Duration::from_secs(30));

        assert_eq!(results, vec![Some(0), Some(1), Some(2)]);
    }

    #[test]
    fn collect_within_leaves_stragglers_empty_but_keeps_the_fast_ones() {
        let tasks: Vec<fn() -> usize> = vec![
            || 10,
            || {
                thread::sleep(Duration::from_secs(30));
                11
            },
            || 12,
        ];

        let results = collect_within(tasks, Duration::from_millis(200));

        // The slow task's slot stays empty; the others survive in order.
        assert_eq!(results, vec![Some(10), None, Some(12)]);
    }
}
