//! Social endpoint configuration (Stage 22a, decision D2 / brief §5, §60, §61).
//!
//! The strong boundary:
//!
//! - There is exactly one production origin, [`PRODUCTION_ORIGIN`] (production's
//!   `DEFAULT_SOCIAL_API_URL`, public client configuration). Nothing else in the code base names it.
//! - The only alternative is [`SocialEndpoint::Test`], whose [`LocalEndpoint`] can only be built
//!   for a **loopback** host (`127.0.0.1`, `::1`, `localhost`) over plain HTTP. There is no way to
//!   point the client at any other remote host - no environment variable, no server payload, no
//!   credential decides it.
//! - The dev/test override `STUDY_NATIVE_SOCIAL_ENDPOINT` accepts only such a loopback URL;
//!   anything else is rejected and Social is disabled for that run (never silently production).
//! - Credentials are bound to an [`EndpointClass`]: an identity made against a local test server
//!   is never sent to production, and vice versa (`social_credentials`).
//! - In test builds the transport additionally refuses every non-loopback connection
//!   (`transport::guard`), so a test cannot reach production even by constructing
//!   `SocialEndpoint::Production` on purpose.

use std::fmt;

/// Production's `DEFAULT_SOCIAL_API_URL` (`desktop/src/lib/social.ts`, `skribbl.ts`).
#[cfg_attr(not(test), allow(dead_code))]
pub const PRODUCTION_ORIGIN: &str = "https://study-tracker-social.danil-poluyanov13.workers.dev";
const PRODUCTION_HOST: &str = "study-tracker-social.danil-poluyanov13.workers.dev";

/// The dev/test override (a loopback URL such as `http://127.0.0.1:47811`).
pub const ENDPOINT_ENV: &str = "STUDY_NATIVE_SOCIAL_ENDPOINT";

/// A loopback-only HTTP endpoint for the local mock Worker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalEndpoint {
    host: String,
    port: u16,
}

fn is_loopback_host(host: &str) -> bool {
    match host {
        "localhost" | "[::1]" => true,
        h => h
            .parse::<std::net::Ipv4Addr>()
            .is_ok_and(|ip| ip.is_loopback()),
    }
}

impl LocalEndpoint {
    /// `None` unless `host` is a loopback address.
    pub fn new(host: &str, port: u16) -> Option<Self> {
        (is_loopback_host(host) && port != 0).then(|| Self {
            host: host.to_string(),
            port,
        })
    }

    /// Parses `http://<loopback host>:<port>` (an optional trailing `/` only - no path, query,
    /// user info or fragment).
    pub fn parse(url: &str) -> Option<Self> {
        let rest = url.trim().strip_prefix("http://")?;
        let rest = rest.strip_suffix('/').unwrap_or(rest);
        if rest.contains(['/', '?', '#', '@']) {
            return None;
        }
        let (host, port) = rest.rsplit_once(':')?;
        Self::new(host, port.parse().ok()?)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn port(&self) -> u16 {
        self.port
    }
}

/// Which family of server an endpoint (or a credential) belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndpointClass {
    Production,
    LocalTest,
}

impl EndpointClass {
    pub fn id(self) -> &'static str {
        match self {
            Self::Production => "production",
            Self::LocalTest => "local-test",
        }
    }

    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "production" => Some(Self::Production),
            "local-test" => Some(Self::LocalTest),
            _ => None,
        }
    }
}

/// Where Social requests go.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SocialEndpoint {
    Production,
    Test(LocalEndpoint),
}

/// A connection target: scheme, host, port. Compared exactly for the image URL policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Origin {
    pub secure: bool,
    pub host: String,
    pub port: u16,
}

impl fmt::Display for Origin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let scheme = if self.secure { "https" } else { "http" };
        let default = if self.secure { 443 } else { 80 };
        if self.port == default {
            write!(f, "{scheme}://{}", self.host)
        } else {
            write!(f, "{scheme}://{}:{}", self.host, self.port)
        }
    }
}

impl Origin {
    pub fn is_loopback(&self) -> bool {
        is_loopback_host(&self.host)
    }
}

/// Why the configured override was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EndpointError {
    /// Set, but not a loopback `http://host:port` URL.
    NotLoopback,
}

impl SocialEndpoint {
    pub fn origin(&self) -> Origin {
        match self {
            Self::Production => Origin {
                secure: true,
                host: PRODUCTION_HOST.to_string(),
                port: 443,
            },
            Self::Test(local) => Origin {
                secure: false,
                host: local.host.clone(),
                port: local.port,
            },
        }
    }

    pub fn class(&self) -> EndpointClass {
        match self {
            Self::Production => EndpointClass::Production,
            Self::Test(_) => EndpointClass::LocalTest,
        }
    }

    /// The endpoint for this run: production, unless `STUDY_NATIVE_SOCIAL_ENDPOINT` names a
    /// loopback mock (a non-loopback value is an error, never a fallback to production).
    pub fn select(override_value: Option<&str>) -> Result<Self, EndpointError> {
        match override_value {
            None => Ok(Self::Production),
            Some(raw) => LocalEndpoint::parse(raw)
                .map(Self::Test)
                .ok_or(EndpointError::NotLoopback),
        }
    }

    pub fn from_env() -> Result<Self, EndpointError> {
        Self::select(std::env::var(ENDPOINT_ENV).ok().as_deref())
    }

    /// A short label for logs and diagnostics (no credentials ever appear in an endpoint).
    pub fn describe(&self) -> String {
        match self {
            Self::Production => "production".to_string(),
            Self::Test(local) => format!("local-test 127.0.0.1-class port {}", local.port),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_override_means_production_and_production_is_the_one_constant() {
        let e = SocialEndpoint::select(None).unwrap();
        assert_eq!(e, SocialEndpoint::Production);
        assert_eq!(e.origin().to_string(), PRODUCTION_ORIGIN);
        assert_eq!(e.class(), EndpointClass::Production);
        assert!(!e.origin().is_loopback());
    }

    #[test]
    fn overrides_must_be_loopback_http() {
        for ok in [
            "http://127.0.0.1:47811",
            "http://127.0.0.1:47811/",
            "http://localhost:9000",
            "http://[::1]:9000",
            "http://127.9.9.9:1",
        ] {
            let e = SocialEndpoint::select(Some(ok)).unwrap_or_else(|_| panic!("{ok}"));
            assert_eq!(e.class(), EndpointClass::LocalTest, "{ok}");
            assert!(e.origin().is_loopback());
        }
        for bad in [
            "",
            "https://127.0.0.1:443",
            "http://example.com:80",
            "http://study-tracker-social.danil-poluyanov13.workers.dev:80",
            PRODUCTION_ORIGIN,
            "http://127.0.0.1",
            "http://127.0.0.1:0",
            "http://127.0.0.1:99999",
            "http://user@127.0.0.1:8080",
            "http://127.0.0.1:8080/path",
            "http://127.0.0.1:8080?x=1",
            "http://127.0.0.1.evil.example:8080",
            "http://10.0.0.1:8080",
            "file:///tmp/x",
            "http://0.0.0.0:8080",
        ] {
            assert_eq!(
                SocialEndpoint::select(Some(bad)),
                Err(EndpointError::NotLoopback),
                "{bad}"
            );
        }
    }

    #[test]
    fn origins_render_without_default_ports() {
        let test = SocialEndpoint::Test(LocalEndpoint::new("127.0.0.1", 47811).unwrap());
        assert_eq!(test.origin().to_string(), "http://127.0.0.1:47811");
        assert!(LocalEndpoint::new("example.com", 80).is_none());
        assert_eq!(
            EndpointClass::parse("local-test"),
            Some(EndpointClass::LocalTest)
        );
        assert_eq!(EndpointClass::parse("prod"), None);
    }
}
