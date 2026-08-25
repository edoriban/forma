/// Pragmatic email check: requires one `@` splitting non-empty local and
/// domain parts; domain must have a dotted suffix with alphanumeric labels.
/// NOT RFC 5322 — quoted locals, IP-literal domains and IDN are rejected.
/// For stricter needs use `.refine(|s| ..)` with a dedicated crate.
pub(crate) fn is_plausible_email(s: &str) -> bool {
    let Some((local, domain)) = s.split_once('@') else {
        return false;
    };
    if s.matches('@').count() != 1 || local.is_empty() || !domain.contains('.') {
        return false;
    }
    if s.chars().any(char::is_whitespace) {
        return false;
    }
    let labels: Vec<&str> = domain.split('.').collect();
    labels.len() >= 2
        && labels
            .iter()
            .all(|l| !l.is_empty() && l.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'))
}

/// Pragmatic URL check: requires `scheme://` prefix, non-empty host, no
/// whitespace. NOT RFC 3986 — percent-encoding validity is not verified.
/// For stricter needs use `.refine(|s| ..)` with the `url` crate via refine.
pub(crate) fn is_plausible_url(s: &str) -> bool {
    let Some((scheme, rest)) = s.split_once("://") else {
        return false;
    };
    if scheme.is_empty()
        || !scheme
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic())
    {
        return false;
    }
    if !scheme
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.')
    {
        return false;
    }
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host = authority.split(':').next().unwrap_or(authority);
    !host.is_empty() && !s.chars().any(char::is_whitespace)
}

/// Pragmatic UUID check: canonical 8-4-4-4-12 hex groups (case-insensitive).
/// Braced/urn forms are rejected in v0.
pub(crate) fn is_plausible_uuid(s: &str) -> bool {
    let groups: Vec<&str> = s.split('-').collect();
    if groups.len() != 5 || !groups.iter().map(|g| g.len()).eq([8usize, 4, 4, 4, 12]) {
        return false;
    }
    groups.iter().all(|g| g.chars().all(|c| c.is_ascii_hexdigit()))
}

#[cfg(test)]
mod tests {
    use crate::validators::{is_plausible_email, is_plausible_url, is_plausible_uuid};

    #[test]
    fn email_fixture_table() {
        let accept = ["user@example.com", "a.b+tag@sub.domain.org", "x@y.co"];
        let reject = [
            "",
            "not-an-email",
            "@example.com",
            "user@",
            "user@@example.com",
            "user@example..com",
            "us er@example.com",
            "user@exa mple.com",
            "user@example.",
            "user@ex\u{e4}mple.com",
        ];
        for s in accept {
            assert!(is_plausible_email(s), "expected accept: {s}");
        }
        for s in reject {
            assert!(!is_plausible_email(s), "expected reject: {s}");
        }
    }

    #[test]
    fn url_fixture_table() {
        let accept = [
            "https://example.com",
            "http://example.com/path?query=1#frag",
            "https://sub.domain.org:8443/x",
            "ftp://files.example.org",
        ];
        let reject = [
            "",
            "example.com",
            "https://",
            "http:///path",
            "://example.com",
            "https://:8080/x",
        ];
        for s in accept {
            assert!(is_plausible_url(s), "expected accept: {s}");
        }
        for s in reject {
            assert!(!is_plausible_url(s), "expected reject: {s}");
        }
    }

    #[test]
    fn uuid_fixture_table() {
        let accept = [
            "123e4567-e89b-12d3-a456-426614174000",
            "00000000-0000-0000-0000-000000000000",
            "123E4567-E89B-12D3-A456-426614174000",
        ];
        let reject = [
            "",
            "123e4567e89b12d3a456426614174000",
            "123e4567-e89b-12d3-a456-42661417400g",
            "123e4567-e89b-12d3-a456",
            "zzzzzzzz-zzzz-zzzz-zzzz-zzzzzzzzzzzz",
        ];
        for s in accept {
            assert!(is_plausible_uuid(s), "expected accept: {s}");
        }
        for s in reject {
            assert!(!is_plausible_uuid(s), "expected reject: {s}");
        }
    }
}
