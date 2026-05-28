use regex::Regex;
use std::sync::LazyLock;

static ZED_TITLE_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(.+?)\s*[—–-]\s*(.+)$").expect("valid zed title regex"));

pub struct ZedContext {
    pub file: String,
    pub project: String,
}

pub fn parse_zed_title(title: &str) -> Option<ZedContext> {
    let caps = ZED_TITLE_RE.captures(title.trim())?;
    let file = caps.get(1)?.as_str().trim().to_string();
    let project = caps.get(2)?.as_str().trim().to_string();

    if file.is_empty() || project.is_empty() {
        return None;
    }

    Some(ZedContext { file, project })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_em_dash_title() {
        let ctx = parse_zed_title("channel.rs — app").unwrap();
        assert_eq!(ctx.file, "channel.rs");
        assert_eq!(ctx.project, "app");
    }

    #[test]
    fn parses_hyphen_title() {
        let ctx = parse_zed_title("main.rs - timetrack").unwrap();
        assert_eq!(ctx.file, "main.rs");
        assert_eq!(ctx.project, "timetrack");
    }

    #[test]
    fn rejects_plain_title() {
        assert!(parse_zed_title("timetrack").is_none());
    }

    #[test]
    fn parses_en_dash_title() {
        let ctx = parse_zed_title("lib.rs – timetrack").unwrap();
        assert_eq!(ctx.file, "lib.rs");
        assert_eq!(ctx.project, "timetrack");
    }

    #[test]
    fn trims_whitespace() {
        let ctx = parse_zed_title("  main.rs  —  timetrack  ").unwrap();
        assert_eq!(ctx.file, "main.rs");
        assert_eq!(ctx.project, "timetrack");
    }

    #[test]
    fn rejects_empty_parts() {
        assert!(parse_zed_title("— timetrack").is_none());
        assert!(parse_zed_title("main.rs —").is_none());
    }
}
