use regex::Regex;
use std::sync::LazyLock;

struct Pattern {
    name: &'static str,
    re: Regex,
}

static PATTERNS: LazyLock<Vec<Pattern>> = LazyLock::new(|| {
    vec![
        Pattern {
            name: "AWS access key",
            re: Regex::new(r"AKIA[0-9A-Z]{16}").expect("valid regex"),
        },
        Pattern {
            name: "GitHub token",
            re: Regex::new(r"ghp_[A-Za-z0-9]{36}").expect("valid regex"),
        },
        Pattern {
            name: "secret key prefix",
            re: Regex::new(r"sk-[A-Za-z0-9]{20,}").expect("valid regex"),
        },
        Pattern {
            name: "bearer token",
            re: Regex::new(r"(?i)bearer\s+[A-Za-z0-9\-._~+/]+=*").expect("valid regex"),
        },
        Pattern {
            name: "private key",
            re: Regex::new(r"-----BEGIN.*PRIVATE KEY").expect("valid regex"),
        },
        Pattern {
            name: "URL with embedded credentials",
            re: Regex::new(r"https?://[^:\s]+:[^@\s]+@").expect("valid regex"),
        },
    ]
});

/// Scan text for secret patterns. Returns the name of the first matched
/// pattern, or None if clean.
pub fn scan(text: &str) -> Option<&'static str> {
    for pattern in PATTERNS.iter() {
        if pattern.re.is_match(text) {
            return Some(pattern.name);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_aws_key() {
        assert_eq!(scan("key AKIAIOSFODNN7EXAMPLE here"), Some("AWS access key"));
    }

    #[test]
    fn detects_github_token() {
        assert_eq!(
            scan("token ghp_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx"),
            Some("GitHub token")
        );
    }

    #[test]
    fn detects_sk_prefix() {
        assert!(scan("sk-abcdefghijklmnopqrstuvwx").is_some());
    }

    #[test]
    fn detects_bearer() {
        assert_eq!(
            scan("Authorization: Bearer eyJhbGciOiJIUzI1NiJ9.payload"),
            Some("bearer token")
        );
    }

    #[test]
    fn detects_private_key() {
        assert_eq!(
            scan("-----BEGIN RSA PRIVATE KEY-----"),
            Some("private key")
        );
    }

    #[test]
    fn detects_url_credentials() {
        assert_eq!(
            scan("https://user:pass@example.com/api"),
            Some("URL with embedded credentials")
        );
    }

    #[test]
    fn clean_text_returns_none() {
        assert_eq!(scan("just a normal papercut about broken tests"), None);
    }
}
