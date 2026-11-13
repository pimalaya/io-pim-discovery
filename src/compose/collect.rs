//! # Config collector
//!
//! [`DiscoveryConfigCollector`] is the pure half of a config compose: consumers
//! run the discovery bricks however they want (sequentially, or in
//! parallel on their own transports) and feed each mechanism's
//! configs in mechanism-priority order; the collector filters them
//! against the requested services and merges duplicates. No I/O
//! happens here, which is what lets orchestration live on the
//! consumer's side.

use alloc::{collections::BTreeSet, vec::Vec};

use crate::compose::config::{DiscoveryService, DiscoveryServiceConfig};

/// Pure accumulator reducing per-mechanism config lists into one
/// deduplicated list.
pub struct DiscoveryConfigCollector {
    services: BTreeSet<DiscoveryService>,
    configs: Vec<DiscoveryServiceConfig>,
}

impl DiscoveryConfigCollector {
    /// Builds a collector restricted to `services` (empty means all
    /// services).
    pub fn new(services: BTreeSet<DiscoveryService>) -> Self {
        Self {
            services,
            configs: Vec::new(),
        }
    }

    /// Whether configs of this service are collected. Orchestrators
    /// use it to skip mechanisms that can only produce filtered-out
    /// services.
    pub fn wants(&self, service: DiscoveryService) -> bool {
        self.services.is_empty() || self.services.contains(&service)
    }

    /// Keeps the configs matching the requested services. A config
    /// whose service, endpoint and username were already collected by
    /// an earlier mechanism merges its authentication methods into the
    /// existing config instead of duplicating it. HTTP endpoints
    /// compare as normalized URLs, and a subdomain of an already
    /// collected host counts as the same service reached through a
    /// rotated backend name, between `https` endpoints only: a
    /// cleartext endpoint never merges with an encrypted one. A
    /// resolved endpoint (RFC 6764) wins over an advertised one, since
    /// an advertised bare origin may not be a DAV context root at all;
    /// between two of the same kind, the parent host wins, since only
    /// it is worth persisting in an account. The source and the trust
    /// follow the endpoint kept.
    pub fn collect(&mut self, configs: Vec<DiscoveryServiceConfig>) {
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
                    let takes_over = if existing.resolved != config.resolved {
                        config.resolved
                    } else {
                        existing.endpoint.subdomain_of(&config.endpoint)
                    };
                    if takes_over {
                        existing.endpoint = config.endpoint;
                        existing.source = config.source;
                        existing.resolved = config.resolved;
                        existing.trust = config.trust;
                    } else if config.resolved && existing.endpoint.equivalent(&config.endpoint) {
                        existing.resolved = true;
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

    /// Whether nothing has been collected yet.
    pub fn is_empty(&self) -> bool {
        self.configs.is_empty()
    }

    /// Returns the collected configs, consuming the collector: the
    /// encrypted endpoints first, each group in collection order.
    pub fn finish(mut self) -> Vec<DiscoveryServiceConfig> {
        self.configs.sort_by_key(|config| !config.is_secure());
        self.configs
    }
}

#[cfg(test)]
mod tests {
    use alloc::{
        collections::BTreeSet,
        string::{String, ToString},
        vec,
        vec::Vec,
    };

    use super::DiscoveryConfigCollector;
    use crate::compose::config::{
        DiscoveryConfigSource, DiscoveryConfigTrust, DiscoveryEndpoint, DiscoverySecurity,
        DiscoveryService, DiscoveryServiceConfig,
    };

    /// A CardDAV config, vouched by the domain when PACC is its source.
    fn carddav(url: &str, source: DiscoveryConfigSource, resolved: bool) -> DiscoveryServiceConfig {
        let trust = match source {
            DiscoveryConfigSource::Pacc => DiscoveryConfigTrust::Domain,
            _ => DiscoveryConfigTrust::Unauthenticated,
        };
        let mut config = DiscoveryServiceConfig::from_dav(DiscoveryService::Carddav, url, trust);
        config.source = source;
        config.resolved = resolved;
        config
    }

    fn endpoint(configs: &[DiscoveryServiceConfig]) -> (String, bool) {
        assert_eq!(configs.len(), 1);
        let DiscoveryEndpoint::Http(url) = &configs[0].endpoint else {
            panic!("HTTP endpoint expected")
        };
        (url.to_string(), configs[0].resolved)
    }

    #[test]
    fn a_resolved_shard_wins_over_its_advertised_parent() {
        // Fastmail: PACC advertises the bare host, RFC 6764 resolves a shard.
        let mut collector = DiscoveryConfigCollector::new(BTreeSet::new());
        collector.collect(vec![carddav(
            "https://carddav.fastmail.com/",
            DiscoveryConfigSource::Pacc,
            false,
        )]);
        collector.collect(vec![carddav(
            "https://d277161.carddav.fastmail.com/dav/addressbooks",
            DiscoveryConfigSource::Dav,
            true,
        )]);
        assert_eq!(
            endpoint(&collector.finish()),
            (
                "https://d277161.carddav.fastmail.com/dav/addressbooks".into(),
                true
            )
        );

        // Both resolved: the parent host wins, as before.
        let mut collector = DiscoveryConfigCollector::new(BTreeSet::new());
        collector.collect(vec![carddav(
            "https://carddav.fastmail.com/dav/addressbooks",
            DiscoveryConfigSource::Pacc,
            true,
        )]);
        collector.collect(vec![carddav(
            "https://d277161.carddav.fastmail.com/dav/addressbooks",
            DiscoveryConfigSource::Dav,
            true,
        )]);
        assert_eq!(
            endpoint(&collector.finish()),
            ("https://carddav.fastmail.com/dav/addressbooks".into(), true)
        );

        // A resolved parent is not replaced by an advertised shard.
        let mut collector = DiscoveryConfigCollector::new(BTreeSet::new());
        collector.collect(vec![carddav(
            "https://carddav.fastmail.com/dav/addressbooks",
            DiscoveryConfigSource::Dav,
            true,
        )]);
        collector.collect(vec![carddav(
            "https://d1.carddav.fastmail.com/",
            DiscoveryConfigSource::Ispdb,
            false,
        )]);
        assert_eq!(
            endpoint(&collector.finish()),
            ("https://carddav.fastmail.com/dav/addressbooks".into(), true)
        );
    }

    #[test]
    fn the_trust_follows_the_endpoint_kept() {
        let pacc = |resolved| {
            carddav(
                "https://carddav.example.com/dav/",
                DiscoveryConfigSource::Pacc,
                resolved,
            )
        };
        let shard = carddav(
            "https://d1.carddav.example.com/dav/",
            DiscoveryConfigSource::Dav,
            true,
        );

        let mut collector = DiscoveryConfigCollector::new(BTreeSet::new());
        collector.collect(vec![pacc(false)]);
        collector.collect(vec![shard.clone()]);
        let configs = collector.finish();
        assert_eq!(configs[0].source, DiscoveryConfigSource::Dav);
        assert_eq!(configs[0].trust, DiscoveryConfigTrust::Unauthenticated);

        let mut collector = DiscoveryConfigCollector::new(BTreeSet::new());
        collector.collect(vec![pacc(true)]);
        collector.collect(vec![shard]);
        let configs = collector.finish();
        assert_eq!(configs[0].source, DiscoveryConfigSource::Pacc);
        assert_eq!(configs[0].trust, DiscoveryConfigTrust::Domain);
    }

    #[test]
    fn a_cleartext_shard_never_replaces_a_tls_endpoint() {
        let mut collector = DiscoveryConfigCollector::new(BTreeSet::new());
        collector.collect(vec![carddav(
            "https://carddav.example.com/",
            DiscoveryConfigSource::Pacc,
            false,
        )]);
        collector.collect(vec![carddav(
            "http://d1.carddav.example.com/dav/",
            DiscoveryConfigSource::Dav,
            true,
        )]);

        let endpoints: Vec<_> = collector
            .finish()
            .into_iter()
            .map(|config| (config.endpoint, config.source))
            .collect();
        assert_eq!(
            endpoints,
            [
                (
                    DiscoveryEndpoint::Http("https://carddav.example.com/".into()),
                    DiscoveryConfigSource::Pacc
                ),
                (
                    DiscoveryEndpoint::Http("http://d1.carddav.example.com/dav/".into()),
                    DiscoveryConfigSource::Dav
                ),
            ]
        );
    }

    #[test]
    fn encrypted_endpoints_come_before_cleartext_ones() {
        let imap = |host: &str, security, source| DiscoveryServiceConfig {
            service: DiscoveryService::Imap,
            endpoint: DiscoveryEndpoint::Tcp {
                host: host.to_string(),
                port: 143,
                security,
            },
            username: None,
            auth: vec![],
            source,
            resolved: false,
            trust: DiscoveryConfigTrust::Unauthenticated,
        };

        let mut collector = DiscoveryConfigCollector::new(BTreeSet::new());
        collector.collect(vec![
            imap(
                "plain.example.com",
                DiscoverySecurity::Plain,
                DiscoveryConfigSource::IspMain,
            ),
            imap(
                "starttls.example.com",
                DiscoverySecurity::Starttls,
                DiscoveryConfigSource::IspMain,
            ),
        ]);
        collector.collect(vec![carddav(
            "http://dav.example.com/dav/",
            DiscoveryConfigSource::Dav,
            true,
        )]);
        collector.collect(vec![imap(
            "tls.example.com",
            DiscoverySecurity::Tls,
            DiscoveryConfigSource::Srv,
        )]);

        let sources: Vec<_> = collector
            .finish()
            .into_iter()
            .map(|config| (config.is_secure(), config.source))
            .collect();
        assert_eq!(
            sources,
            [
                (true, DiscoveryConfigSource::IspMain),
                (true, DiscoveryConfigSource::Srv),
                (false, DiscoveryConfigSource::IspMain),
                (false, DiscoveryConfigSource::Dav),
            ]
        );
    }
}
