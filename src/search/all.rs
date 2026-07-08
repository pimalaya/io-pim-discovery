//! # Search-all coroutine
//!
//! [`SearchAll`] turns one email address into every [`ServiceConfig`]
//! the known mechanisms can produce, in order: fixed provider rules
//! (domain match, then MX-based detection for custom domains hosted
//! on Google Workspace or Microsoft 365), PACC, the Mozilla
//! autoconfig locations (ISP main, ISP fallback, the mailconf TXT
//! redirect, ISPDB), RFC 6186 SRV records, the RFC 6764
//! CalDAV/CardDAV resolve and the RFC 8620 JMAP resolve. A final probe
//! then asks each collected HTTP endpoint which authentication schemes
//! it actually advertises on its unauthenticated 401 (PACC §5.4.2) and
//! refines the config's password and bearer methods accordingly:
//! account-level documents cannot express per-service schemes, the
//! endpoint itself can.
//!
//! Mechanism failures are logged and skipped: only an invalid email
//! address fails the whole search. Mechanisms irrelevant to the
//! requested services are skipped entirely, and configs for the same
//! service, endpoint and username merge their authentication methods
//! instead of duplicating (the first mechanism keeps the source tag).

use core::mem;

use alloc::{
    collections::BTreeSet,
    string::{String, ToString},
    vec::Vec,
};

use log::{debug, trace};
use thiserror::Error;
use url::Url;

use crate::{
    autoconfig::{isp::DiscoveryIsp, mailconf::DiscoveryMailconf, mx::DiscoveryDnsMx},
    coroutine::{DiscoveryCoroutine, DiscoveryCoroutineState, DiscoveryYield},
    pacc::discover::DiscoveryPacc,
    rfc6186::discover::DiscoverySrv,
    rfc6764::{resolve::ResolveDav, types::DavService},
    rfc8620::resolve::ResolveJmap,
    rfc9110::ProbeAuth,
    search::{
        providers::Provider,
        types::{ConfigSource, Endpoint, Service, ServiceConfig},
    },
};

/// Errors emitted by the search coroutines.
#[derive(Debug, Error)]
pub enum SearchError {
    /// The input is not a valid `local@domain` email address.
    #[error("Search email `{0}` is missing the `@` separator")]
    InvalidEmail(String),
}

/// I/O-free coroutine that collects every service config the known
/// mechanisms produce for one email address.
pub struct SearchAll {
    email: String,
    domain: String,
    services: BTreeSet<Service>,
    resolver: Url,
    first: bool,
    provider_matched: bool,
    configs: Vec<ServiceConfig>,
    state: State,
}

impl SearchAll {
    /// Builds a search for `email`, restricted to `services` (empty
    /// means all services). `resolver` must be a `tcp://host:port`
    /// DNS-over-TCP resolver URL.
    pub fn new(
        email: impl AsRef<str>,
        services: BTreeSet<Service>,
        resolver: Url,
    ) -> Result<Self, SearchError> {
        Self::build(email, services, resolver, false)
    }

    pub(crate) fn build(
        email: impl AsRef<str>,
        services: BTreeSet<Service>,
        resolver: Url,
        first: bool,
    ) -> Result<Self, SearchError> {
        let email = email.as_ref().trim();

        let Some((_, domain)) = email.split_once('@') else {
            return Err(SearchError::InvalidEmail(email.to_string()));
        };

        debug!("begin config search");
        trace!("email {email}, first: {first}, services: {services:?}");

        Ok(Self {
            email: email.to_string(),
            domain: domain.trim_matches('.').to_ascii_lowercase(),
            services,
            resolver,
            first,
            provider_matched: false,
            configs: Vec::new(),
            state: State::Start,
        })
    }

    fn wants(&self, service: Service) -> bool {
        self.services.is_empty() || self.services.contains(&service)
    }

    fn wants_mail(&self) -> bool {
        [Service::Imap, Service::Pop3, Service::Smtp]
            .iter()
            .any(|s| self.wants(*s))
    }

    /// Keeps the configs matching the requested services. A config
    /// whose service, endpoint and username were already collected by
    /// an earlier mechanism merges its authentication methods into the
    /// existing config instead of duplicating it. HTTP endpoints
    /// compare as normalized URLs, and a subdomain of an already
    /// collected host counts as the same service reached through a
    /// rotated backend name: the parent host wins the endpoint, since
    /// only it is worth persisting in an account.
    fn collect(&mut self, configs: Vec<ServiceConfig>) {
        for config in configs {
            if !self.wants(config.service) {
                continue;
            }

            let existing = self.configs.iter_mut().find(|c| {
                c.service == config.service
                    && c.username == config.username
                    && (c.endpoint.equivalent(&config.endpoint)
                        || c.endpoint.subdomain_of(&config.endpoint)
                        || config.endpoint.subdomain_of(&c.endpoint))
            });

            match existing {
                Some(existing) => {
                    if existing.endpoint.subdomain_of(&config.endpoint) {
                        existing.endpoint = config.endpoint;
                        existing.source = config.source;
                    }
                    for method in config.auth {
                        if !existing.auth.contains(&method) {
                            existing.auth.push(method);
                        }
                    }
                }
                None => self.configs.push(config),
            }
        }
    }

    /// Starts the next endpoint auth probe of the queue, or ends the
    /// search when every target is exhausted.
    fn probe_next(
        &mut self,
        mut queue: Vec<ProbeTask>,
    ) -> DiscoveryCoroutineState<DiscoveryYield, Result<Vec<ServiceConfig>, SearchError>> {
        loop {
            let Some(mut task) = queue.pop() else {
                return self.advance(Step::End);
            };
            if task.urls.is_empty() {
                continue;
            }

            let url = task.urls.remove(0);
            debug!("probe endpoint authentication schemes");
            trace!("{url}");

            let probe = ProbeAuth::new(url);
            self.state = State::Probe { queue, task, probe };
            return self.resume(None);
        }
    }

    /// Enters the first applicable mechanism at or after `step`, or
    /// completes when none is left. In first mode, the auth probe runs
    /// as soon as any config has been collected and the search
    /// completes right after it.
    fn advance(
        &mut self,
        mut step: Step,
    ) -> DiscoveryCoroutineState<DiscoveryYield, Result<Vec<ServiceConfig>, SearchError>> {
        if self.first && !self.configs.is_empty() && !matches!(step, Step::Probe | Step::End) {
            debug!("stop search at first mechanism yielding configs");
            trace!("{:?}", self.configs);
            step = Step::Probe;
        }

        loop {
            match step {
                Step::Mx => {
                    if !self.provider_matched {
                        let mx = DiscoveryDnsMx::new(&self.domain, self.resolver.clone());
                        self.state = State::Mx(mx);
                        return self.resume(None);
                    }
                    step = Step::Pacc;
                }
                Step::Pacc => match DiscoveryPacc::new(&self.domain, self.resolver.clone()) {
                    Ok(pacc) => {
                        self.state = State::Pacc(pacc);
                        return self.resume(None);
                    }
                    Err(err) => {
                        debug!("skip PACC discovery");
                        trace!("{err:?}");
                        step = Step::IspMain;
                    }
                },
                Step::IspMain => {
                    if self.wants_mail() {
                        let local_part = self.email.split_once('@').map(|(l, _)| l);
                        let url = DiscoveryIsp::main_url(
                            local_part.unwrap_or_default(),
                            &self.domain,
                            true,
                        );

                        match url {
                            Ok(url) => {
                                self.state = State::IspMain(DiscoveryIsp::new(url));
                                return self.resume(None);
                            }
                            Err(err) => {
                                debug!("skip autoconfig ISP main URL");
                                trace!("{err:?}");
                            }
                        }
                    }
                    step = Step::IspFallback;
                }
                Step::IspFallback => {
                    if self.wants_mail() {
                        match DiscoveryIsp::fallback_url(&self.domain, true) {
                            Ok(url) => {
                                self.state = State::IspFallback(DiscoveryIsp::new(url));
                                return self.resume(None);
                            }
                            Err(err) => {
                                debug!("skip autoconfig ISP fallback URL");
                                trace!("{err:?}");
                            }
                        }
                    }
                    step = Step::Mailconf;
                }
                Step::Mailconf => {
                    if self.wants_mail() {
                        let mailconf = DiscoveryMailconf::new(&self.domain, self.resolver.clone());
                        self.state = State::Mailconf(mailconf);
                        return self.resume(None);
                    }
                    step = Step::Ispdb;
                }
                Step::Ispdb => {
                    if self.wants_mail() {
                        match DiscoveryIsp::db_url(&self.domain, true) {
                            Ok(url) => {
                                self.state = State::Ispdb(DiscoveryIsp::new(url));
                                return self.resume(None);
                            }
                            Err(err) => {
                                debug!("skip autoconfig ISPDB URL");
                                trace!("{err:?}");
                            }
                        }
                    }
                    step = Step::Srv;
                }
                Step::Srv => {
                    if self.wants(Service::Imap) || self.wants(Service::Smtp) {
                        let srv = DiscoverySrv::new(&self.domain, self.resolver.clone());
                        self.state = State::Srv(srv);
                        return self.resume(None);
                    }
                    step = Step::Caldav;
                }
                Step::Caldav => {
                    if self.wants(Service::Caldav) {
                        let resolve = ResolveDav::new(
                            &self.domain,
                            DavService::Caldav,
                            self.resolver.clone(),
                        );
                        self.state = State::Caldav(resolve);
                        return self.resume(None);
                    }
                    step = Step::Carddav;
                }
                Step::Carddav => {
                    if self.wants(Service::Carddav) {
                        let resolve = ResolveDav::new(
                            &self.domain,
                            DavService::Carddav,
                            self.resolver.clone(),
                        );
                        self.state = State::Carddav(resolve);
                        return self.resume(None);
                    }
                    step = Step::Jmap;
                }
                Step::Jmap => {
                    if self.wants(Service::Jmap) {
                        let resolve = ResolveJmap::new(&self.domain, self.resolver.clone());
                        self.state = State::Jmap(resolve);
                        return self.resume(None);
                    }
                    step = Step::Probe;
                }
                Step::Probe => {
                    let queue: Vec<ProbeTask> = self
                        .configs
                        .iter()
                        .enumerate()
                        .map(|(index, config)| ProbeTask {
                            index,
                            urls: probe_urls(config),
                        })
                        .filter(|task| !task.urls.is_empty())
                        .collect();

                    if queue.is_empty() {
                        step = Step::End;
                        continue;
                    }
                    return self.probe_next(queue);
                }
                Step::End => {
                    debug!("end of config search");
                    trace!("{:?}", self.configs);
                    return DiscoveryCoroutineState::Complete(Ok(mem::take(&mut self.configs)));
                }
            }
        }
    }
}

impl DiscoveryCoroutine for SearchAll {
    type Yield = DiscoveryYield;
    type Return = Result<Vec<ServiceConfig>, SearchError>;

    fn resume(&mut self, arg: Option<&[u8]>) -> DiscoveryCoroutineState<Self::Yield, Self::Return> {
        match mem::take(&mut self.state) {
            State::Start => {
                if let Some(provider) = Provider::from_domain(&self.domain) {
                    debug!("email domain matched a fixed provider rule");
                    trace!("{} -> {provider:?}", self.domain);
                    self.provider_matched = true;
                    let configs = provider.configs(&self.email);
                    self.collect(configs);
                }
                self.advance(Step::Mx)
            }
            State::Mx(mut mx) => match mx.resume(arg) {
                DiscoveryCoroutineState::Yielded(y) => {
                    self.state = State::Mx(mx);
                    DiscoveryCoroutineState::Yielded(y)
                }
                DiscoveryCoroutineState::Complete(res) => {
                    match res {
                        Ok(records) => {
                            for record in records {
                                let exchange = record.data().exchange().to_string();

                                if let Some(provider) = Provider::from_mx(&exchange) {
                                    debug!("MX record matched a fixed provider rule");
                                    trace!("{exchange} -> {provider:?}");
                                    let configs = provider.configs(&self.email);
                                    self.collect(configs);
                                    break;
                                }
                            }
                        }
                        Err(err) => {
                            debug!("skip MX provider detection");
                            trace!("{err:?}");
                        }
                    }
                    self.advance(Step::Pacc)
                }
            },
            State::Pacc(mut pacc) => match pacc.resume(arg) {
                DiscoveryCoroutineState::Yielded(y) => {
                    self.state = State::Pacc(pacc);
                    DiscoveryCoroutineState::Yielded(y)
                }
                DiscoveryCoroutineState::Complete(res) => {
                    match res {
                        Ok(config) => self.collect(ServiceConfig::from_pacc(&config)),
                        Err(err) => {
                            debug!("skip PACC discovery");
                            trace!("{err:?}");
                        }
                    }
                    self.advance(Step::IspMain)
                }
            },
            State::IspMain(mut isp) => match isp.resume(arg) {
                DiscoveryCoroutineState::Yielded(y) => {
                    self.state = State::IspMain(isp);
                    DiscoveryCoroutineState::Yielded(y)
                }
                DiscoveryCoroutineState::Complete(res) => {
                    match res {
                        Ok(config) => self.collect(ServiceConfig::from_autoconfig(
                            &config,
                            &self.email,
                            ConfigSource::IspMain,
                        )),
                        Err(err) => {
                            debug!("skip autoconfig ISP main URL");
                            trace!("{err:?}");
                        }
                    }
                    self.advance(Step::IspFallback)
                }
            },
            State::IspFallback(mut isp) => match isp.resume(arg) {
                DiscoveryCoroutineState::Yielded(y) => {
                    self.state = State::IspFallback(isp);
                    DiscoveryCoroutineState::Yielded(y)
                }
                DiscoveryCoroutineState::Complete(res) => {
                    match res {
                        Ok(config) => self.collect(ServiceConfig::from_autoconfig(
                            &config,
                            &self.email,
                            ConfigSource::IspFallback,
                        )),
                        Err(err) => {
                            debug!("skip autoconfig ISP fallback URL");
                            trace!("{err:?}");
                        }
                    }
                    self.advance(Step::Mailconf)
                }
            },
            State::Mailconf(mut mailconf) => match mailconf.resume(arg) {
                DiscoveryCoroutineState::Yielded(y) => {
                    self.state = State::Mailconf(mailconf);
                    DiscoveryCoroutineState::Yielded(y)
                }
                DiscoveryCoroutineState::Complete(Ok(url)) => {
                    debug!("follow mailconf TXT redirect");
                    trace!("{url}");
                    self.state = State::MailconfIsp(DiscoveryIsp::new(url));
                    self.resume(None)
                }
                DiscoveryCoroutineState::Complete(Err(err)) => {
                    debug!("skip mailconf TXT redirect");
                    trace!("{err:?}");
                    self.advance(Step::Ispdb)
                }
            },
            State::MailconfIsp(mut isp) => match isp.resume(arg) {
                DiscoveryCoroutineState::Yielded(y) => {
                    self.state = State::MailconfIsp(isp);
                    DiscoveryCoroutineState::Yielded(y)
                }
                DiscoveryCoroutineState::Complete(res) => {
                    match res {
                        Ok(config) => self.collect(ServiceConfig::from_autoconfig(
                            &config,
                            &self.email,
                            ConfigSource::Mailconf,
                        )),
                        Err(err) => {
                            debug!("skip mailconf autoconfig document");
                            trace!("{err:?}");
                        }
                    }
                    self.advance(Step::Ispdb)
                }
            },
            State::Ispdb(mut isp) => match isp.resume(arg) {
                DiscoveryCoroutineState::Yielded(y) => {
                    self.state = State::Ispdb(isp);
                    DiscoveryCoroutineState::Yielded(y)
                }
                DiscoveryCoroutineState::Complete(res) => {
                    match res {
                        Ok(config) => self.collect(ServiceConfig::from_autoconfig(
                            &config,
                            &self.email,
                            ConfigSource::Ispdb,
                        )),
                        Err(err) => {
                            debug!("skip autoconfig ISPDB");
                            trace!("{err:?}");
                        }
                    }
                    self.advance(Step::Srv)
                }
            },
            State::Srv(mut srv) => match srv.resume(arg) {
                DiscoveryCoroutineState::Yielded(y) => {
                    self.state = State::Srv(srv);
                    DiscoveryCoroutineState::Yielded(y)
                }
                DiscoveryCoroutineState::Complete(res) => {
                    match res {
                        Ok(report) => self.collect(ServiceConfig::from_srv(&report)),
                        Err(err) => {
                            debug!("skip RFC 6186 SRV discovery");
                            trace!("{err:?}");
                        }
                    }
                    self.advance(Step::Caldav)
                }
            },
            State::Caldav(mut resolve) => match resolve.resume(arg) {
                DiscoveryCoroutineState::Yielded(y) => {
                    self.state = State::Caldav(resolve);
                    DiscoveryCoroutineState::Yielded(y)
                }
                DiscoveryCoroutineState::Complete(res) => {
                    match res {
                        Ok(url) => {
                            self.collect(vec![ServiceConfig::from_dav(Service::Caldav, url)])
                        }
                        Err(err) => {
                            debug!("skip RFC 6764 CalDAV resolve");
                            trace!("{err:?}");
                        }
                    }
                    self.advance(Step::Carddav)
                }
            },
            State::Carddav(mut resolve) => match resolve.resume(arg) {
                DiscoveryCoroutineState::Yielded(y) => {
                    self.state = State::Carddav(resolve);
                    DiscoveryCoroutineState::Yielded(y)
                }
                DiscoveryCoroutineState::Complete(res) => {
                    match res {
                        Ok(url) => {
                            self.collect(vec![ServiceConfig::from_dav(Service::Carddav, url)])
                        }
                        Err(err) => {
                            debug!("skip RFC 6764 CardDAV resolve");
                            trace!("{err:?}");
                        }
                    }
                    self.advance(Step::Jmap)
                }
            },
            State::Jmap(mut resolve) => match resolve.resume(arg) {
                DiscoveryCoroutineState::Yielded(y) => {
                    self.state = State::Jmap(resolve);
                    DiscoveryCoroutineState::Yielded(y)
                }
                DiscoveryCoroutineState::Complete(res) => {
                    match res {
                        Ok(session) => self.collect(vec![ServiceConfig::from_jmap(
                            session.url,
                            &session.auth_schemes,
                        )]),
                        Err(err) => {
                            debug!("skip RFC 8620 JMAP resolve");
                            trace!("{err:?}");
                        }
                    }
                    self.advance(Step::Probe)
                }
            },
            State::Probe {
                mut queue,
                task,
                mut probe,
            } => match probe.resume(arg) {
                DiscoveryCoroutineState::Yielded(y) => {
                    self.state = State::Probe { queue, task, probe };
                    DiscoveryCoroutineState::Yielded(y)
                }
                DiscoveryCoroutineState::Complete(res) => {
                    match res {
                        Ok(schemes) if !schemes.is_empty() => {
                            if let Some(config) = self.configs.get_mut(task.index) {
                                config.refine_auth(&schemes);
                            }
                        }
                        // Nothing learned at this URL: the task's next
                        // URL (the well-known walk) gets its turn.
                        Ok(_) => queue.push(task),
                        Err(err) => {
                            debug!("skip failed auth probe");
                            trace!("{err:?}");
                            queue.push(task);
                        }
                    }
                    self.probe_next(queue)
                }
            },
            State::Done => panic!("SearchAll::resume called after completion"),
        }
    }
}

/// The ordered mechanism chain; [`SearchAll::advance`] walks it from
/// a given step, skipping mechanisms irrelevant to the requested
/// services.
#[derive(Clone, Copy)]
enum Step {
    Mx,
    Pacc,
    IspMain,
    IspFallback,
    Mailconf,
    Ispdb,
    Srv,
    Caldav,
    Carddav,
    Jmap,
    Probe,
    End,
}

#[derive(Default)]
enum State {
    Start,
    Mx(DiscoveryDnsMx),
    Pacc(DiscoveryPacc),
    IspMain(DiscoveryIsp),
    IspFallback(DiscoveryIsp),
    Mailconf(DiscoveryMailconf),
    MailconfIsp(DiscoveryIsp),
    Ispdb(DiscoveryIsp),
    Srv(DiscoverySrv),
    Caldav(ResolveDav),
    Carddav(ResolveDav),
    Jmap(ResolveJmap),
    Probe {
        queue: Vec<ProbeTask>,
        task: ProbeTask,
        probe: ProbeAuth,
    },
    #[default]
    Done,
}

/// One config's pending auth probe: its index in the collected list
/// and the URLs left to try, in order.
struct ProbeTask {
    index: usize,
    urls: Vec<Url>,
}

/// The URLs whose unauthenticated 401 may advertise the config's
/// schemes: the HTTP endpoint itself, then the service's well-known
/// path for the DAV services (some servers, fastmail among them, 404
/// the bare origin but guard the well-known walk).
fn probe_urls(config: &ServiceConfig) -> Vec<Url> {
    let Endpoint::Http(raw) = &config.endpoint else {
        return Vec::new();
    };
    let Ok(url) = Url::parse(raw) else {
        return Vec::new();
    };

    let mut urls = vec![url.clone()];
    let well_known = match config.service {
        Service::Caldav => Some("/.well-known/caldav"),
        Service::Carddav => Some("/.well-known/carddav"),
        _ => None,
    };
    if let Some(path) = well_known {
        let mut probe = url;
        probe.set_path(path);
        urls.push(probe);
    }

    urls
}
