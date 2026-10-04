//! # Discovered service configuration
//!
//! The compose module reduces every discovery mechanism to one common
//! output: a list of [`DiscoveryServiceConfig`], each describing one way to
//! reach one service (endpoint, login, authentication methods),
//! tagged with the mechanism that produced it. Conversion helpers on
//! [`DiscoveryServiceConfig`] flatten each mechanism's native document into
//! that shape.

use alloc::{
    string::{String, ToString},
    vec::Vec,
};

use serde::{Deserialize, Serialize};
use url::Url;

#[cfg(feature = "autoconfig")]
use crate::autoconfig::config::{
    DiscoveryAuthenticationType, DiscoveryAutoconfig, DiscoverySecurityType, DiscoveryServerType,
};
use crate::compose::providers::DiscoveryKnownProvider;
#[cfg(feature = "pacc")]
use crate::pacc::config::DiscoveryPaccConfig;
#[cfg(feature = "rfc6186")]
use crate::rfc6186::service::{DiscoverySrvReport, DiscoverySrvService};

/// One discovered way to use one service: where to connect, how to
/// authenticate, and which mechanism found it.
///
/// It serializes with its `source` as a string and, when a fixed rule
/// matched, the provider in a `provider` field of its own.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(
    into = "DiscoveryServiceConfigWire",
    try_from = "DiscoveryServiceConfigWire"
)]
pub struct DiscoveryServiceConfig {
    /// The service this config describes.
    pub service: DiscoveryService,

    /// Where to reach the service.
    pub endpoint: DiscoveryEndpoint,

    /// The login to present, when the mechanism advertises one
    /// (autoconfig placeholders already substituted).
    pub username: Option<String>,

    /// The authentication methods the service accepts.
    pub auth: Vec<DiscoveryAuthMethod>,

    /// The mechanism that produced this config.
    pub source: DiscoveryConfigSource,

    /// Whether the endpoint came out of an RFC 6764 resolution (the
    /// `dav` mechanism's walk, or the well-known probe of an advertised
    /// bare origin) rather than as a mechanism advertised it.
    pub resolved: bool,
}

impl DiscoveryServiceConfig {
    /// Whether the endpoint is reached encrypted: a TCP endpoint over
    /// TLS or STARTTLS, or an `https` URL.
    pub fn is_secure(&self) -> bool {
        match &self.endpoint {
            DiscoveryEndpoint::Tcp { security, .. } => *security != DiscoverySecurity::Plain,
            DiscoveryEndpoint::Http(url) => {
                Url::parse(url).is_ok_and(|url| url.scheme() == "https")
            }
        }
    }

    /// The URLs whose unauthenticated 401 may advertise the config's
    /// schemes (to feed [`refine_auth`]): the HTTP endpoint itself,
    /// then the service's well-known path for the DAV services (some
    /// servers, fastmail among them, 404 the bare origin but guard
    /// the well-known walk). Empty for TCP endpoints and for provider
    /// APIs, whose fixed rules already name their OAuth grants.
    ///
    /// [`refine_auth`]: Self::refine_auth
    pub fn probe_urls(&self) -> Vec<Url> {
        let probed = matches!(
            self.service,
            DiscoveryService::Jmap
                | DiscoveryService::Caldav
                | DiscoveryService::Carddav
                | DiscoveryService::Webdav
        );
        if !probed {
            return Vec::new();
        }

        let DiscoveryEndpoint::Http(raw) = &self.endpoint else {
            return Vec::new();
        };
        let Ok(url) = Url::parse(raw) else {
            return Vec::new();
        };

        let mut urls = vec![url.clone()];
        let well_known = match self.service {
            DiscoveryService::Caldav => Some("/.well-known/caldav"),
            DiscoveryService::Carddav => Some("/.well-known/carddav"),
            _ => None,
        };
        if let Some(path) = well_known {
            let mut probe = url;
            probe.set_path(path);
            urls.push(probe);
        }

        urls
    }

    /// Refines the password and bearer methods from the schemes the
    /// service endpoint advertised on its unauthenticated 401 (PACC
    /// §5.4.2): the endpoint's own advertisement beats any
    /// account-level claim, since a provider may take passwords on one
    /// service and only bearer tokens on another (fastmail does).
    /// OAuth methods stay untouched (they describe how to obtain a
    /// token, not a scheme), and schemes naming neither `basic` nor
    /// `bearer` leave the config as discovered.
    pub fn refine_auth(&mut self, schemes: &[String]) {
        let mut auth = Vec::new();

        for scheme in schemes {
            match scheme.as_str() {
                "basic" => auth.push(DiscoveryAuthMethod::Password),
                "bearer" => auth.push(DiscoveryAuthMethod::Bearer),
                _ => (),
            }
        }
        if auth.is_empty() {
            return;
        }

        for method in self.auth.drain(..) {
            let probed = matches!(
                method,
                DiscoveryAuthMethod::Password | DiscoveryAuthMethod::Bearer
            );
            if !probed && !auth.contains(&method) {
                auth.push(method);
            }
        }
        self.auth = auth;
    }

    /// Flattens a Mozilla autoconfig document into one config per
    /// incoming/outgoing server. Servers without a hostname are
    /// skipped; a missing port falls back to the well-known port of
    /// the service and security combination.
    #[cfg(feature = "autoconfig")]
    pub fn from_autoconfig(
        config: &DiscoveryAutoconfig,
        email: &str,
        source: DiscoveryConfigSource,
    ) -> Vec<Self> {
        let provider = &config.email_provider;
        let servers = provider
            .incoming_server
            .iter()
            .chain(&provider.outgoing_server);

        let mut configs = Vec::new();

        for server in servers {
            let Some(hostname) = &server.hostname else {
                continue;
            };

            let service = match server.r#type {
                DiscoveryServerType::Imap => DiscoveryService::Imap,
                DiscoveryServerType::Pop3 => DiscoveryService::Pop3,
                DiscoveryServerType::Smtp => DiscoveryService::Smtp,
            };

            let security = match server.socket_type {
                Some(DiscoverySecurityType::Tls) | None => DiscoverySecurity::Tls,
                Some(DiscoverySecurityType::Starttls) => DiscoverySecurity::Starttls,
                Some(DiscoverySecurityType::Plain) => DiscoverySecurity::Plain,
            };

            let Some(port) = server.port.or(default_port(service, security)) else {
                continue;
            };

            let mut auth = Vec::new();

            for method in &server.authentication {
                let method = match method {
                    DiscoveryAuthenticationType::PasswordCleartext
                    | DiscoveryAuthenticationType::PasswordEncrypted => {
                        DiscoveryAuthMethod::Password
                    }
                    DiscoveryAuthenticationType::OAuth2 => {
                        let Some(oauth) = &config.oauth2 else {
                            continue;
                        };

                        DiscoveryAuthMethod::OauthAuthorizationCodeGrant {
                            authorization_endpoint: oauth.auth_url.clone(),
                            token_endpoint: oauth.token_url.clone(),
                            scope: Some(oauth.scope.clone()),
                        }
                    }
                    _ => continue,
                };

                if !auth.contains(&method) {
                    auth.push(method);
                }
            }

            configs.push(Self {
                service,
                endpoint: DiscoveryEndpoint::Tcp {
                    host: substitute(hostname, email),
                    port,
                    security,
                },
                username: server.username.as_deref().map(|u| substitute(u, email)),
                auth,
                source,
                resolved: false,
            });
        }

        configs
    }

    /// Flattens a PACC document into one config per advertised
    /// protocol. PACC mandates implicit TLS for the text protocols,
    /// so their configs use the well-known implicit-TLS ports.
    #[cfg(feature = "pacc")]
    pub fn from_pacc(config: &DiscoveryPaccConfig) -> Vec<Self> {
        let mut auth = Vec::new();

        if let Some(oauth) = &config.authentication.oauth_public {
            auth.push(DiscoveryAuthMethod::OauthIssuer(oauth.issuer.clone()));
        }

        if config.authentication.password == Some(true) {
            auth.push(DiscoveryAuthMethod::Password);
        }

        let protocols = &config.protocols;
        let mut configs = Vec::new();

        let tcp_protocols = [
            (DiscoveryService::Imap, &protocols.imap, 993),
            (DiscoveryService::Pop3, &protocols.pop3, 995),
            (DiscoveryService::Smtp, &protocols.smtp, 465),
            (DiscoveryService::Managesieve, &protocols.managesieve, 4190),
        ];

        for (service, protocol, port) in tcp_protocols {
            let Some(protocol) = protocol else {
                continue;
            };

            configs.push(Self {
                service,
                endpoint: DiscoveryEndpoint::Tcp {
                    host: protocol.host.clone(),
                    port,
                    security: DiscoverySecurity::Tls,
                },
                username: None,
                auth: auth.clone(),
                source: DiscoveryConfigSource::Pacc,
                resolved: false,
            });
        }

        let http_protocols = [
            (DiscoveryService::Jmap, &protocols.jmap),
            (DiscoveryService::Caldav, &protocols.caldav),
            (DiscoveryService::Carddav, &protocols.carddav),
            (DiscoveryService::Webdav, &protocols.webdav),
        ];

        for (service, protocol) in http_protocols {
            let Some(protocol) = protocol else {
                continue;
            };

            configs.push(Self {
                service,
                endpoint: DiscoveryEndpoint::Http(protocol.url.clone()),
                username: None,
                auth: auth.clone(),
                source: DiscoveryConfigSource::Pacc,
                resolved: false,
            });
        }

        configs
    }

    /// Converts an RFC 6186 SRV report into configs. SRV records
    /// advertise no authentication data, so password login is
    /// assumed; `_imaps` and `_submissions` (RFC 8314) map to implicit
    /// TLS, `_imap` and `_submission` to STARTTLS.
    #[cfg(feature = "rfc6186")]
    pub fn from_srv(report: &DiscoverySrvReport) -> Vec<Self> {
        let services = [
            (
                DiscoveryService::Imap,
                &report.imaps,
                DiscoverySecurity::Tls,
            ),
            (
                DiscoveryService::Imap,
                &report.imap,
                DiscoverySecurity::Starttls,
            ),
            (
                DiscoveryService::Smtp,
                &report.submissions,
                DiscoverySecurity::Tls,
            ),
            (
                DiscoveryService::Smtp,
                &report.submission,
                DiscoverySecurity::Starttls,
            ),
        ];

        let mut configs = Vec::new();

        for (service, record, security) in services {
            // NOTE: an SRV target of `.` means the service is
            // explicitly not available (RFC 2782); the target comes
            // in with its trailing dot already trimmed.
            let Some(DiscoverySrvService { host, port, .. }) = record else {
                continue;
            };
            if host.is_empty() {
                continue;
            }

            configs.push(Self {
                service,
                endpoint: DiscoveryEndpoint::Tcp {
                    host: host.clone(),
                    port: *port,
                    security,
                },
                username: None,
                auth: vec![DiscoveryAuthMethod::Password],
                source: DiscoveryConfigSource::Srv,
                resolved: false,
            });
        }

        configs
    }

    /// Wraps an RFC 6764 context root into a single config. DAV
    /// discovery advertises no authentication data, so password login
    /// is assumed.
    pub fn from_dav(service: DiscoveryService, url: impl ToString) -> Self {
        Self {
            service,
            endpoint: DiscoveryEndpoint::Http(url.to_string()),
            username: None,
            auth: vec![DiscoveryAuthMethod::Password],
            source: DiscoveryConfigSource::Dav,
            resolved: true,
        }
    }

    /// Wraps an RFC 8620 JMAP session URL into a single config. The
    /// authentication methods derive from the schemes the session
    /// endpoint advertised on its unauthenticated 401 (`basic` means
    /// password login, `bearer` a bearer token). With no advertisement
    /// the list stays empty: which methods to offer then is the
    /// consumer's call, not a fact discovery found.
    pub fn from_jmap(url: impl ToString, schemes: &[String]) -> Self {
        let mut auth = Vec::new();

        for scheme in schemes {
            match scheme.as_str() {
                "basic" => auth.push(DiscoveryAuthMethod::Password),
                "bearer" => auth.push(DiscoveryAuthMethod::Bearer),
                _ => (),
            }
        }

        Self {
            service: DiscoveryService::Jmap,
            endpoint: DiscoveryEndpoint::Http(url.to_string()),
            username: None,
            auth,
            source: DiscoveryConfigSource::Jmap,
            resolved: false,
        }
    }
}

/// A PIM service kind.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DiscoveryService {
    /// IMAP mailbox access (RFC 9051).
    Imap,
    /// POP3 mailbox access (RFC 1939).
    Pop3,
    /// SMTP mail submission (RFC 5321).
    Smtp,
    /// JMAP (RFC 8620).
    Jmap,
    /// CalDAV calendar access (RFC 4791).
    Caldav,
    /// CardDAV contact access (RFC 6352).
    Carddav,
    /// Generic WebDAV (RFC 4918).
    Webdav,
    /// ManageSieve server-side filtering (RFC 5804).
    Managesieve,
    /// Gmail API mail access.
    Gmail,
    /// Google Calendar API calendar access.
    Gcal,
    /// Google People API contact access.
    Gpeople,
    /// Microsoft Graph mail access.
    Msgraph,
    /// Microsoft Graph calendar access.
    MsgraphCalendar,
    /// Microsoft Graph contact access.
    MsgraphContacts,
}

/// Where to reach a service.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DiscoveryEndpoint {
    /// Text protocol endpoint (IMAP, POP3, SMTP, ManageSieve).
    Tcp {
        /// Hostname or IP address of the server.
        host: String,
        /// TCP port number.
        port: u16,
        /// Transport security negotiation mode.
        security: DiscoverySecurity,
    },
    /// HTTP endpoint (JMAP, CalDAV, CardDAV, WebDAV, provider APIs).
    Http(String),
}

impl DiscoveryEndpoint {
    /// Reports whether two endpoints reach the same service: byte
    /// equality, or normalized-URL equality for HTTP endpoints, so
    /// mechanisms disagreeing only on a trailing slash or an explicit
    /// default port still merge.
    pub fn equivalent(&self, other: &Self) -> bool {
        if self == other {
            return true;
        }

        match (self, other) {
            (Self::Http(a), Self::Http(b)) => match (Url::parse(a), Url::parse(b)) {
                (Ok(a), Ok(b)) => a == b,
                _ => false,
            },
            _ => false,
        }
    }

    /// Reports whether this HTTP endpoint's host is a subdomain of the
    /// other's: the mark of a rotated backend behind a provider's
    /// stable service name (fastmail's SRV records answer with
    /// `dNNNNNN.carddav.fastmail.com` shards under the
    /// `carddav.fastmail.com` its own configuration document
    /// advertises).
    pub fn subdomain_of(&self, other: &Self) -> bool {
        let (Self::Http(a), Self::Http(b)) = (self, other) else {
            return false;
        };
        let (Ok(a), Ok(b)) = (Url::parse(a), Url::parse(b)) else {
            return false;
        };
        let (Some(a), Some(b)) = (a.host_str(), b.host_str()) else {
            return false;
        };

        a.len() > b.len() && a.ends_with(b) && a.as_bytes()[a.len() - b.len() - 1] == b'.'
    }
}

#[cfg(test)]
mod tests {
    use alloc::{string::ToString, vec};

    use serde_json::{Value, from_value, to_value};

    use crate::compose::{
        config::{
            DiscoveryAuthMethod, DiscoveryConfigSource, DiscoveryEndpoint, DiscoverySecurity,
            DiscoveryService, DiscoveryServiceConfig,
        },
        providers::DiscoveryKnownProvider,
    };

    #[cfg(feature = "rfc6186")]
    #[test]
    fn implicit_tls_submission_srv_becomes_a_tls_smtp_config() {
        use crate::rfc6186::service::{DiscoverySrvReport, DiscoverySrvService};

        // A domain that publishes only `_submissions._tcp` (RFC 8314
        // implicit TLS, port 465), mirroring `_imaps` on the send side.
        let report = DiscoverySrvReport {
            imaps: Some(DiscoverySrvService {
                host: "imap.migadu.com".to_string(),
                port: 993,
                priority: 0,
                weight: 1,
            }),
            submissions: Some(DiscoverySrvService {
                host: "smtp.migadu.com".to_string(),
                port: 465,
                priority: 0,
                weight: 1,
            }),
            ..Default::default()
        };

        let configs = DiscoveryServiceConfig::from_srv(&report);

        // The submission record must yield an SMTP endpoint at the
        // advertised host over implicit TLS, not fall through to a
        // guessed `smtp.<domain>` downstream.
        let smtp = configs
            .iter()
            .find(|config| config.service == DiscoveryService::Smtp)
            .expect("`_submissions` must produce an SMTP config");
        assert_eq!(
            smtp.endpoint,
            DiscoveryEndpoint::Tcp {
                host: "smtp.migadu.com".to_string(),
                port: 465,
                security: DiscoverySecurity::Tls,
            },
        );
        assert_eq!(smtp.source, DiscoveryConfigSource::Srv);
    }

    #[test]
    fn probed_schemes_beat_account_level_claims() {
        let mut config = DiscoveryServiceConfig {
            service: DiscoveryService::Jmap,
            endpoint: DiscoveryEndpoint::Http("https://api.example.com/jmap/session".to_string()),
            username: None,
            auth: vec![
                DiscoveryAuthMethod::OauthIssuer("https://api.example.com".to_string()),
                DiscoveryAuthMethod::Password,
            ],
            source: DiscoveryConfigSource::Pacc,
            resolved: false,
        };

        // The endpoint advertises bearer only: the account-level
        // password claim goes, the OAuth issuer stays.
        config.refine_auth(&["bearer".to_string()]);
        assert_eq!(
            config.auth,
            vec![
                DiscoveryAuthMethod::Bearer,
                DiscoveryAuthMethod::OauthIssuer("https://api.example.com".to_string()),
            ],
        );

        // Unknown schemes leave the config as discovered.
        config.refine_auth(&["negotiate".to_string()]);
        assert!(config.auth.contains(&DiscoveryAuthMethod::Bearer));
    }

    #[test]
    fn plain_and_http_endpoints_are_not_secure() {
        let config = |endpoint| DiscoveryServiceConfig {
            service: DiscoveryService::Imap,
            endpoint,
            username: None,
            auth: vec![],
            source: DiscoveryConfigSource::IspMain,
            resolved: false,
        };
        let tcp = |security| DiscoveryEndpoint::Tcp {
            host: "imap.example.com".to_string(),
            port: 143,
            security,
        };
        let http = |url: &str| DiscoveryEndpoint::Http(url.to_string());

        assert!(!config(tcp(DiscoverySecurity::Plain)).is_secure());
        assert!(config(tcp(DiscoverySecurity::Starttls)).is_secure());
        assert!(config(tcp(DiscoverySecurity::Tls)).is_secure());
        assert!(!config(http("http://dav.example.com/")).is_secure());
        assert!(config(http("https://dav.example.com/")).is_secure());
    }

    #[test]
    fn json_names_the_source_and_the_provider_apart() {
        let provider = DiscoveryKnownProvider::Google
            .configs("a@gmail.com")
            .remove(0);
        let autoconfig = DiscoveryServiceConfig {
            service: DiscoveryService::Imap,
            endpoint: DiscoveryEndpoint::Tcp {
                host: "imap.example.com".to_string(),
                port: 993,
                security: DiscoverySecurity::Tls,
            },
            username: Some("a@example.com".to_string()),
            auth: vec![DiscoveryAuthMethod::OauthDeviceAuthorizationGrant {
                device_authorization_endpoint: "https://example.com/device".to_string(),
                token_endpoint: "https://example.com/token".to_string(),
                scope: None,
            }],
            source: DiscoveryConfigSource::IspMain,
            resolved: false,
        };

        let provider_json = to_value(&provider).unwrap();
        assert_eq!(provider_json["source"], "provider");
        assert_eq!(provider_json["provider"], "google");

        let autoconfig_json = to_value(&autoconfig).unwrap();
        assert_eq!(autoconfig_json["source"], "ispMain");
        assert!(autoconfig_json.get("provider").is_none());
        assert_eq!(autoconfig_json["resolved"], false);
        let dav = to_value(DiscoveryServiceConfig::from_dav(
            DiscoveryService::Carddav,
            "https://dav.example.com/dav",
        ))
        .unwrap();
        assert_eq!(dav["resolved"], true);
        assert!(from_value::<DiscoveryServiceConfig>(dav).unwrap().resolved);

        no_snake_case_key(&provider_json);
        no_snake_case_key(&autoconfig_json);

        let back: DiscoveryServiceConfig = from_value(provider_json).unwrap();
        assert_eq!(back.source, provider.source);
        assert_eq!(back.auth, provider.auth);
    }

    fn no_snake_case_key(value: &Value) {
        match value {
            Value::Object(map) => {
                for (key, value) in map {
                    assert!(!key.contains('_'), "snake_case key `{key}`");
                    no_snake_case_key(value);
                }
            }
            Value::Array(values) => values.iter().for_each(no_snake_case_key),
            _ => (),
        }
    }

    #[test]
    fn http_endpoints_compare_normalized() {
        let bare = DiscoveryEndpoint::Http("https://carddav.example.com".to_string());
        let slash = DiscoveryEndpoint::Http("https://carddav.example.com/".to_string());
        let port = DiscoveryEndpoint::Http("https://carddav.example.com:443/".to_string());
        let other = DiscoveryEndpoint::Http("https://carddav.example.com/dav".to_string());

        assert!(bare.equivalent(&slash));
        assert!(bare.equivalent(&port));
        assert!(!bare.equivalent(&other));
    }

    #[test]
    fn subdomain_marks_a_rotated_backend() {
        let parent = DiscoveryEndpoint::Http("https://carddav.example.com".to_string());
        let shard = DiscoveryEndpoint::Http("https://d063023.carddav.example.com/dav".to_string());
        let sibling = DiscoveryEndpoint::Http("https://caldav.example.com".to_string());
        let lookalike = DiscoveryEndpoint::Http("https://evilcarddav.example.com".to_string());

        assert!(shard.subdomain_of(&parent));
        assert!(!parent.subdomain_of(&shard));
        assert!(!sibling.subdomain_of(&parent));
        assert!(!lookalike.subdomain_of(&parent));
    }
}

/// Transport security of a TCP service endpoint.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DiscoverySecurity {
    /// Unencrypted connection.
    Plain,
    /// Opportunistic TLS via the STARTTLS command.
    Starttls,
    /// Implicit TLS from the first byte.
    Tls,
}

/// How a client can authenticate against a service.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum DiscoveryAuthMethod {
    /// Username and password login (possibly an app password).
    Password,

    /// Bearer token (RFC 6750), e.g. a provider-issued API token.
    Bearer,

    /// OAuth 2.0 authorization code grant (RFC 6749 §4.1).
    OauthAuthorizationCodeGrant {
        /// URL of the authorization server's authorization endpoint.
        authorization_endpoint: String,
        /// URL of the authorization server's token endpoint.
        token_endpoint: String,
        /// Space-delimited set of requested scopes, if known.
        scope: Option<String>,
    },

    /// OAuth 2.0 device authorization grant (RFC 8628).
    OauthDeviceAuthorizationGrant {
        /// URL of the device authorization endpoint.
        device_authorization_endpoint: String,
        /// URL of the authorization server's token endpoint.
        token_endpoint: String,
        /// Space-delimited set of requested scopes, if known.
        scope: Option<String>,
    },

    /// OAuth 2.0 with grant types not known yet: the issuer's RFC
    /// 8414 authorization server metadata lists the endpoints and
    /// supported grants.
    OauthIssuer(String),
}

/// The mechanism that produced a config.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DiscoveryConfigSource {
    /// A fixed provider rule (domain or MX match).
    Provider(DiscoveryKnownProvider),
    /// PACC discovery.
    Pacc,
    /// The autoconfig ISP main URL.
    IspMain,
    /// The autoconfig ISP fallback URL.
    IspFallback,
    /// The autoconfig document behind the mailconf TXT redirect.
    Mailconf,
    /// The Thunderbird ISPDB.
    Ispdb,
    /// RFC 6186 SRV records.
    Srv,
    /// RFC 6764 CalDAV/CardDAV resolve.
    Dav,
    /// RFC 8620 JMAP resolve.
    Jmap,
}

/// The serialized shape of a [`DiscoveryServiceConfig`].
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DiscoveryServiceConfigWire {
    service: DiscoveryService,
    endpoint: DiscoveryEndpoint,
    username: Option<String>,
    auth: Vec<DiscoveryAuthMethod>,
    source: DiscoveryConfigSourceWire,
    #[serde(default)]
    resolved: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    provider: Option<DiscoveryKnownProvider>,
}

/// The serialized [`DiscoveryConfigSource`], the provider left out.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum DiscoveryConfigSourceWire {
    Provider,
    Pacc,
    IspMain,
    IspFallback,
    Mailconf,
    Ispdb,
    Srv,
    Dav,
    Jmap,
}

impl From<DiscoveryServiceConfig> for DiscoveryServiceConfigWire {
    fn from(config: DiscoveryServiceConfig) -> Self {
        let (source, provider) = match config.source {
            DiscoveryConfigSource::Provider(p) => (DiscoveryConfigSourceWire::Provider, Some(p)),
            DiscoveryConfigSource::Pacc => (DiscoveryConfigSourceWire::Pacc, None),
            DiscoveryConfigSource::IspMain => (DiscoveryConfigSourceWire::IspMain, None),
            DiscoveryConfigSource::IspFallback => (DiscoveryConfigSourceWire::IspFallback, None),
            DiscoveryConfigSource::Mailconf => (DiscoveryConfigSourceWire::Mailconf, None),
            DiscoveryConfigSource::Ispdb => (DiscoveryConfigSourceWire::Ispdb, None),
            DiscoveryConfigSource::Srv => (DiscoveryConfigSourceWire::Srv, None),
            DiscoveryConfigSource::Dav => (DiscoveryConfigSourceWire::Dav, None),
            DiscoveryConfigSource::Jmap => (DiscoveryConfigSourceWire::Jmap, None),
        };

        Self {
            service: config.service,
            endpoint: config.endpoint,
            username: config.username,
            auth: config.auth,
            source,
            resolved: config.resolved,
            provider,
        }
    }
}

impl TryFrom<DiscoveryServiceConfigWire> for DiscoveryServiceConfig {
    type Error = &'static str;

    fn try_from(wire: DiscoveryServiceConfigWire) -> Result<Self, Self::Error> {
        let source = match wire.source {
            DiscoveryConfigSourceWire::Provider => match wire.provider {
                Some(provider) => DiscoveryConfigSource::Provider(provider),
                None => return Err("Provider source without a provider field"),
            },
            DiscoveryConfigSourceWire::Pacc => DiscoveryConfigSource::Pacc,
            DiscoveryConfigSourceWire::IspMain => DiscoveryConfigSource::IspMain,
            DiscoveryConfigSourceWire::IspFallback => DiscoveryConfigSource::IspFallback,
            DiscoveryConfigSourceWire::Mailconf => DiscoveryConfigSource::Mailconf,
            DiscoveryConfigSourceWire::Ispdb => DiscoveryConfigSource::Ispdb,
            DiscoveryConfigSourceWire::Srv => DiscoveryConfigSource::Srv,
            DiscoveryConfigSourceWire::Dav => DiscoveryConfigSource::Dav,
            DiscoveryConfigSourceWire::Jmap => DiscoveryConfigSource::Jmap,
        };

        Ok(Self {
            service: wire.service,
            endpoint: wire.endpoint,
            username: wire.username,
            auth: wire.auth,
            source,
            resolved: wire.resolved,
        })
    }
}

/// Substitutes the Mozilla autoconfig placeholders (%EMAILADDRESS%,
/// %EMAILLOCALPART%, %EMAILDOMAIN%) in a hostname or username value.
#[cfg(feature = "autoconfig")]
fn substitute(value: &str, email: &str) -> String {
    let (local_part, domain) = email.split_once('@').unwrap_or((email, ""));

    value
        .replace("%EMAILADDRESS%", email)
        .replace("%EMAILLOCALPART%", local_part)
        .replace("%EMAILDOMAIN%", domain)
}

/// Returns the well-known port for a service and security
/// combination, used when a mechanism omits the port.
#[cfg(feature = "autoconfig")]
fn default_port(service: DiscoveryService, security: DiscoverySecurity) -> Option<u16> {
    match (service, security) {
        (DiscoveryService::Imap, DiscoverySecurity::Tls) => Some(993),
        (DiscoveryService::Imap, _) => Some(143),
        (DiscoveryService::Pop3, DiscoverySecurity::Tls) => Some(995),
        (DiscoveryService::Pop3, _) => Some(110),
        (DiscoveryService::Smtp, DiscoverySecurity::Tls) => Some(465),
        (DiscoveryService::Smtp, DiscoverySecurity::Starttls) => Some(587),
        (DiscoveryService::Smtp, DiscoverySecurity::Plain) => Some(25),
        (DiscoveryService::Managesieve, _) => Some(4190),
        _ => None,
    }
}
