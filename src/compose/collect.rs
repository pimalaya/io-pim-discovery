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
    /// rotated backend name. A resolved endpoint (RFC 6764) wins over
    /// an advertised one, since an advertised bare origin may not be a
    /// DAV context root at all; between two of the same kind, the
    /// parent host wins, since only it is worth persisting in an
    /// account.
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

    /// Returns the collected configs, consuming the collector.
    pub fn finish(self) -> Vec<DiscoveryServiceConfig> {
        self.configs
    }
}

#[cfg(test)]
mod tests {
    use alloc::{
        collections::BTreeSet,
        string::{String, ToString},
        vec,
    };

    use super::DiscoveryConfigCollector;
    use crate::compose::config::{
        DiscoveryConfigSource, DiscoveryEndpoint, DiscoveryService, DiscoveryServiceConfig,
    };

    fn carddav(url: &str, source: DiscoveryConfigSource, resolved: bool) -> DiscoveryServiceConfig {
        let mut config = DiscoveryServiceConfig::from_dav(DiscoveryService::Carddav, url);
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
}
