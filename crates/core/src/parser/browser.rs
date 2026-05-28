pub fn normalize_url(url: &str) -> Option<String> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return None;
    }

    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        Some(trimmed.to_string())
    } else if trimmed.contains('.') && !trimmed.contains(' ') {
        Some(format!("https://{trimmed}"))
    } else {
        None
    }
}

pub fn display_host(url: &str) -> String {
    url.trim_start_matches("https://")
        .trim_start_matches("http://")
        .split('/')
        .next()
        .unwrap_or(url)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_bare_domain() {
        assert_eq!(
            normalize_url("github.com/user/repo").as_deref(),
            Some("https://github.com/user/repo")
        );
    }

    #[test]
    fn keeps_https_url() {
        assert_eq!(
            normalize_url("https://docs.rs/trait.Iterator").as_deref(),
            Some("https://docs.rs/trait.Iterator")
        );
    }

    #[test]
    fn returns_none_for_empty_or_invalid() {
        assert_eq!(normalize_url(""), None);
        assert_eq!(normalize_url("   "), None);
        assert_eq!(normalize_url("not a url"), None);
    }

    #[test]
    fn display_host_strips_scheme_and_path() {
        assert_eq!(
            display_host("https://github.com/user/repo/pull/1"),
            "github.com"
        );
    }
}
