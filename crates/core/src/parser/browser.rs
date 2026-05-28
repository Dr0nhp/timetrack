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

/// Human-readable label for browser tabs (e.g. Google Chat contact name).
pub fn display_label(url: Option<&str>, page_title: Option<&str>, window_title: &str) -> Option<String> {
    let url = url?;
    if !is_gmail_chat_url(url) {
        return None;
    }

    if let Some(title) = page_title.filter(|title| is_useful_title(title, url)) {
        return Some(format_gmail_chat_label(title));
    }

    if is_useful_title(window_title, url) {
        return Some(format_gmail_chat_label(window_title));
    }

    Some(classify_gmail_chat_url(url))
}

fn is_gmail_chat_url(url: &str) -> bool {
    url.contains("mail.google.com") && url.contains("#chat/")
}

fn is_useful_title(title: &str, url: &str) -> bool {
    let title = title.trim();
    !title.is_empty() && title != url && !title.starts_with("http")
}

fn format_gmail_chat_label(title: &str) -> String {
    let name = title
        .trim()
        .strip_suffix(" - Gmail")
        .or_else(|| title.strip_suffix(" – Gmail"))
        .or_else(|| title.strip_suffix(" - Google Mail"))
        .unwrap_or(title)
        .trim();

    if name.is_empty() {
        "Google Chat".into()
    } else {
        format!("Google Chat · {name}")
    }
}

fn classify_gmail_chat_url(url: &str) -> String {
    if url.contains("#chat/dm/") {
        "Google Chat · Direktnachricht".into()
    } else if url.contains("#chat/space/") {
        "Google Chat · Gruppe".into()
    } else {
        "Google Chat".into()
    }
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

    #[test]
    fn display_label_uses_window_title_for_gmail_chat() {
        assert_eq!(
            display_label(
                Some("https://mail.google.com/mail/u/0/#chat/dm/5dErPSAAAAE"),
                None,
                "Alice Example - Gmail"
            ),
            Some("Google Chat · Alice Example".into())
        );
    }

    #[test]
    fn display_label_falls_back_for_gmail_chat_without_title() {
        assert_eq!(
            display_label(
                Some("https://mail.google.com/mail/u/0/#chat/dm/5dErPSAAAAE"),
                None,
                ""
            ),
            Some("Google Chat · Direktnachricht".into())
        );
    }
}
