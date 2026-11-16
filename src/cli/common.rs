//! Shared CLI helpers: the DNS resolver flag, the per-domain service
//! sets, and the service-config list output.

use std::{
    fmt,
    string::{String, ToString},
    vec::Vec,
};

use anyhow::Result;
use clap::Args;
use pimalaya_cli::{columns::Columns, sanitize};
use pimalaya_stream::tls::Tls;

use crate::{
    compose::{
        client::DiscoveryComposeClientStd,
        config::{
            DiscoveryAuthMethod, DiscoveryConfigSource, DiscoveryConfigTrust, DiscoveryEndpoint,
            DiscoverySecurity, DiscoveryService, DiscoveryServiceConfig,
        },
        providers::DiscoveryKnownProvider,
    },
    shared::dns::{DNS_SERVER, resolver_url},
};

/// Services of the email domain (JMAP included: it serves mail too;
/// ManageSieve manages server-side mail filters).
pub const EMAIL: &[DiscoveryService] = &[
    DiscoveryService::Imap,
    DiscoveryService::Pop3,
    DiscoveryService::Smtp,
    DiscoveryService::Jmap,
    DiscoveryService::Managesieve,
    DiscoveryService::Gmail,
    DiscoveryService::Msgraph,
];

/// Services of the calendar domain (JMAP reused across domains).
pub const CALENDAR: &[DiscoveryService] = &[
    DiscoveryService::Caldav,
    DiscoveryService::Jmap,
    DiscoveryService::Gcal,
    DiscoveryService::MsgraphCalendar,
];

/// Services of the contact domain (JMAP reused across domains).
pub const CONTACT: &[DiscoveryService] = &[
    DiscoveryService::Carddav,
    DiscoveryService::Jmap,
    DiscoveryService::Gpeople,
    DiscoveryService::MsgraphContacts,
];

/// Services of the file domain (generic WebDAV file storage).
pub const FILE: &[DiscoveryService] = &[DiscoveryService::Webdav];

/// Network flags shared by every discovery command: the DNS resolver
/// and the secure-only mode.
#[derive(Debug, Args)]
pub struct ServerArg {
    /// DNS resolver: `host:port`, or an RFC 8484 resolver URL such as
    /// `https://cloudflare-dns.com/dns-query`.
    #[arg(long, default_value = DNS_SERVER)]
    pub server: String,
    /// Refuse plain HTTP requests and unencrypted endpoints (STARTTLS
    /// kept). DNS lookups still go to the resolver, over TCP or RFC 8484
    /// HTTPS; a plain `http://` resolver is refused.
    #[arg(long)]
    pub secure_only: bool,
}

impl ServerArg {
    /// Builds a compose client resolving DNS through the flag and
    /// running the HTTPS mechanisms over `tls`.
    pub fn client(&self, tls: &Tls) -> Result<DiscoveryComposeClientStd> {
        let dns = resolver_url(&self.server)?;
        let client = DiscoveryComposeClientStd::new(dns, tls.clone());
        Ok(client.with_secure_only(self.secure_only))
    }

    /// Same as [`client`](Self::client), then checks that the resolver
    /// answers for the domain of `input` (an address or a domain), so
    /// a resolver failure is an error rather than an empty list.
    pub fn checked_client(&self, tls: &Tls, input: &str) -> Result<DiscoveryComposeClientStd> {
        let client = self.client(tls)?;
        client.check_resolver(input)?;
        Ok(client)
    }
}

/// Keeps only the configs whose service belongs to `services`.
pub fn only(
    configs: Vec<DiscoveryServiceConfig>,
    services: &[DiscoveryService],
) -> Vec<DiscoveryServiceConfig> {
    configs
        .into_iter()
        .filter(|config| services.contains(&config.service))
        .collect()
}

/// The service-config list output, wrapping a raw (unmerged) config
/// list.
#[derive(serde::Serialize)]
#[serde(transparent)]
pub struct ConfigsOutput(pub Vec<DiscoveryServiceConfig>);

impl fmt::Display for ConfigsOutput {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", columns(&self.0))
    }
}

/// Renders a service-config list as aligned columns: SERVICE,
/// ENDPOINT, USERNAME, the SOURCE mechanism that produced each row, its
/// TRUST, then AUTH, the longest, last.
///
/// The endpoint, the username and the auth methods come from the
/// network, so they are sanitized before they reach the terminal.
pub fn columns(configs: &[DiscoveryServiceConfig]) -> Columns<'static> {
    let header = ["SERVICE", "ENDPOINT", "USERNAME", "SOURCE", "TRUST", "AUTH"];
    let mut columns = Columns::new(header);

    for config in configs {
        let endpoint = endpoint_label(&config.endpoint);
        let username = config.username.as_deref().unwrap_or_default();
        let auth = auth_label(&config.auth);

        columns.push([
            service_name(config.service),
            sanitize::text(&endpoint).as_ref(),
            sanitize::text(username).as_ref(),
            source_name(config.source),
            trust_name(config.trust),
            sanitize::text(&auth).as_ref(),
        ]);
    }

    columns
}

/// Wire name of a service.
pub fn service_name(service: DiscoveryService) -> &'static str {
    match service {
        DiscoveryService::Imap => "imap",
        DiscoveryService::Pop3 => "pop3",
        DiscoveryService::Smtp => "smtp",
        DiscoveryService::Jmap => "jmap",
        DiscoveryService::Caldav => "caldav",
        DiscoveryService::Carddav => "carddav",
        DiscoveryService::Webdav => "webdav",
        DiscoveryService::Managesieve => "managesieve",
        DiscoveryService::Gmail => "gmail",
        DiscoveryService::Gcal => "gcal",
        DiscoveryService::Gpeople => "gpeople",
        DiscoveryService::Msgraph => "msgraph",
        DiscoveryService::MsgraphCalendar => "msgraphCalendar",
        DiscoveryService::MsgraphContacts => "msgraphContacts",
    }
}

fn endpoint_label(endpoint: &DiscoveryEndpoint) -> String {
    match endpoint {
        DiscoveryEndpoint::Tcp {
            host,
            port,
            security,
        } => {
            let security = match security {
                DiscoverySecurity::Plain => "plain",
                DiscoverySecurity::Starttls => "STARTTLS",
                DiscoverySecurity::Tls => "SSL",
            };
            format!("{host}:{port} ({security})")
        }
        DiscoveryEndpoint::Http(url) => url.clone(),
    }
}

fn auth_label(methods: &[DiscoveryAuthMethod]) -> String {
    if methods.is_empty() {
        return "-".to_string();
    }

    methods
        .iter()
        .map(|method| match method {
            DiscoveryAuthMethod::Password => "password".to_string(),
            DiscoveryAuthMethod::Bearer => "bearer".to_string(),
            DiscoveryAuthMethod::OauthAuthorizationCodeGrant { .. } => {
                "oauth2:authorization-code".to_string()
            }
            DiscoveryAuthMethod::OauthDeviceAuthorizationGrant { .. } => {
                "oauth2:device".to_string()
            }
            DiscoveryAuthMethod::OauthIssuer(issuer) => format!("oauth2:{issuer}"),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn source_name(source: DiscoveryConfigSource) -> &'static str {
    match source {
        DiscoveryConfigSource::Provider(DiscoveryKnownProvider::Google) => "provider:google",
        DiscoveryConfigSource::Provider(DiscoveryKnownProvider::Microsoft) => "provider:microsoft",
        DiscoveryConfigSource::Pacc => "pacc",
        DiscoveryConfigSource::IspMain => "isp",
        DiscoveryConfigSource::IspFallback => "isp-fallback",
        DiscoveryConfigSource::Mailconf => "mailconf",
        DiscoveryConfigSource::Ispdb => "ispdb",
        DiscoveryConfigSource::Mx => "mx",
        DiscoveryConfigSource::Srv => "srv",
        DiscoveryConfigSource::Dav => "dav",
        DiscoveryConfigSource::Jmap => "jmap",
    }
}

fn trust_name(trust: DiscoveryConfigTrust) -> &'static str {
    match trust {
        DiscoveryConfigTrust::Rule => "rule",
        DiscoveryConfigTrust::Domain => "domain",
        DiscoveryConfigTrust::ThirdParty => "third-party",
        DiscoveryConfigTrust::Unauthenticated => "unauthenticated",
    }
}

#[cfg(test)]
mod tests {
    use std::{string::ToString, vec};

    use crate::{
        cli::common::ConfigsOutput,
        compose::config::{
            DiscoveryAuthMethod, DiscoveryConfigSource, DiscoveryConfigTrust, DiscoveryEndpoint,
            DiscoverySecurity, DiscoveryService, DiscoveryServiceConfig,
        },
    };

    #[test]
    fn prints_configs_as_columns_with_network_values_sanitized() {
        let configs = vec![
            DiscoveryServiceConfig {
                service: DiscoveryService::Imap,
                endpoint: DiscoveryEndpoint::Tcp {
                    host: "imap.example.org".into(),
                    port: 993,
                    security: DiscoverySecurity::Tls,
                },
                username: Some("a\u{1b}[2Jb".into()),
                auth: vec![DiscoveryAuthMethod::Password],
                source: DiscoveryConfigSource::Srv,
                resolved: false,
                trust: DiscoveryConfigTrust::Unauthenticated,
            },
            DiscoveryServiceConfig {
                service: DiscoveryService::Jmap,
                endpoint: DiscoveryEndpoint::Http("https://example.org/jmap".into()),
                username: None,
                auth: vec![],
                source: DiscoveryConfigSource::Jmap,
                resolved: false,
                trust: DiscoveryConfigTrust::Domain,
            },
        ];

        let expected = "\
SERVICE  ENDPOINT                    USERNAME  SOURCE  TRUST            AUTH
imap     imap.example.org:993 (SSL)  a\u{fffd}[2Jb    srv     unauthenticated  password
jmap     https://example.org/jmap    -         jmap    domain           -
";
        assert_eq!(ConfigsOutput(configs).to_string(), expected);
    }
}
