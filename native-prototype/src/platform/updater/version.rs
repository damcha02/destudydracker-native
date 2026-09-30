//! Semantic versions (semver 2.0.0 precedence), for "is the feed newer than me?".

use std::cmp::Ordering;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
    /// Pre-release identifiers (`1.2.3-beta.2` -> `["beta", "2"]`); empty for a release.
    pub pre: Vec<String>,
}

impl Version {
    /// Parses `MAJOR.MINOR.PATCH[-PRERELEASE][+BUILD]`, with an optional leading `v`. Build
    /// metadata is validated and discarded (it never affects precedence). Anything else - a
    /// missing component, a leading zero, an empty identifier, a stray character - is `None`.
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        let text = text.strip_prefix('v').unwrap_or(text);
        let (core_and_pre, build) = match text.split_once('+') {
            Some((a, b)) => (a, Some(b)),
            None => (text, None),
        };
        if let Some(build) = build {
            if build.is_empty() || !build.split('.').all(valid_identifier) {
                return None;
            }
        }
        let (core, pre) = match core_and_pre.split_once('-') {
            Some((core, pre)) => (core, Some(pre)),
            None => (core_and_pre, None),
        };
        let mut numbers = core.split('.');
        let major = numeric(numbers.next()?)?;
        let minor = numeric(numbers.next()?)?;
        let patch = numeric(numbers.next()?)?;
        if numbers.next().is_some() {
            return None;
        }
        let pre = match pre {
            None => Vec::new(),
            Some(pre) => {
                let ids: Vec<&str> = pre.split('.').collect();
                if ids.iter().any(|id| {
                    !valid_identifier(id) || (is_numeric(id) && id.len() > 1 && id.starts_with('0'))
                }) {
                    return None;
                }
                ids.into_iter().map(str::to_string).collect()
            }
        };
        Some(Self {
            major,
            minor,
            patch,
            pre,
        })
    }
}

fn is_numeric(id: &str) -> bool {
    !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit())
}

fn valid_identifier(id: &str) -> bool {
    !id.is_empty() && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

fn numeric(part: &str) -> Option<u64> {
    if !is_numeric(part) || (part.len() > 1 && part.starts_with('0')) {
        return None;
    }
    part.parse().ok()
}

impl fmt::Display for Version {
    /// Normalized form; only `[0-9A-Za-z.-]` can appear, which is what makes it safe to embed in a
    /// file name (no path separators, no `..` components beyond literal dots in identifiers).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)?;
        if !self.pre.is_empty() {
            write!(f, "-{}", self.pre.join("."))?;
        }
        Ok(())
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        (self.major, self.minor, self.patch)
            .cmp(&(other.major, other.minor, other.patch))
            .then_with(|| match (self.pre.is_empty(), other.pre.is_empty()) {
                (true, true) => Ordering::Equal,
                (true, false) => Ordering::Greater, // a release outranks its pre-releases
                (false, true) => Ordering::Less,
                (false, false) => compare_pre(&self.pre, &other.pre),
            })
    }
}

fn compare_pre(a: &[String], b: &[String]) -> Ordering {
    for (x, y) in a.iter().zip(b) {
        let ord = match (is_numeric(x), is_numeric(y)) {
            (true, true) => x.len().cmp(&y.len()).then_with(|| x.cmp(y)), // numeric, no leading zeros
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            (false, false) => x.cmp(y),
        };
        if ord != Ordering::Equal {
            return ord;
        }
    }
    a.len().cmp(&b.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &str) -> Version {
        Version::parse(s).unwrap_or_else(|| panic!("{s} should parse"))
    }

    #[test]
    fn parses_production_style_versions_and_a_leading_v() {
        assert_eq!(v("0.1.67").to_string(), "0.1.67");
        assert_eq!(v("v0.2.0").to_string(), "0.2.0");
        assert_eq!(v("1.0.0-beta.2+build.5").to_string(), "1.0.0-beta.2");
    }

    #[test]
    fn rejects_malformed_versions() {
        for bad in [
            "",
            "1",
            "1.2",
            "1.2.3.4",
            "01.2.3",
            "1.02.3",
            "1.2.x",
            "a.b.c",
            "1.2.3-",
            "1.2.3-a..b",
            "1.2.3-01",
            "1.2.3+",
            "1.2.3 4",
            "../1.2.3",
            "1.2.3/../x",
            "-1.2.3",
            "1.2.3-é",
        ] {
            assert!(Version::parse(bad).is_none(), "{bad:?} must be rejected");
        }
    }

    #[test]
    fn precedence_follows_semver() {
        assert!(v("0.1.67") > v("0.1.66"));
        assert!(v("0.2.0") > v("0.1.99"));
        assert!(v("1.0.0") > v("0.99.99"));
        assert!(v("0.1.10") > v("0.1.9"), "numeric, not lexicographic");
        assert_eq!(
            v("0.1.67"),
            v("v0.1.67+other-build"),
            "build metadata is ignored"
        );
        // semver.org's own example chain
        let chain = [
            "1.0.0-alpha",
            "1.0.0-alpha.1",
            "1.0.0-alpha.beta",
            "1.0.0-beta",
            "1.0.0-beta.2",
            "1.0.0-beta.11",
            "1.0.0-rc.1",
            "1.0.0",
        ];
        for pair in chain.windows(2) {
            assert!(v(pair[0]) < v(pair[1]), "{} < {}", pair[0], pair[1]);
        }
    }

    #[test]
    fn display_never_contains_path_separators() {
        for s in ["1.2.3", "1.2.3-rc.1", "1.2.3-x-y.z"] {
            let shown = v(s).to_string();
            assert!(!shown.contains('/') && !shown.contains('\\') && !shown.contains(':'));
        }
    }
}
