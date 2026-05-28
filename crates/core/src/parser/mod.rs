pub mod browser;
pub mod terminal;
pub mod zed;

use crate::models::{ActivityContext, ActivitySnapshot};

const ZED_BUNDLE_IDS: &[&str] = &["dev.zed.Zed", "dev.zed.Zed-Preview"];
const TERMINAL_BUNDLE_IDS: &[&str] = &[
    "com.apple.Terminal",
    "com.googlecode.iterm2",
    "dev.warp.Warp-Stable",
    "dev.warp.Warp",
    "net.kovidgoyal.kitty",
    "com.github.wez.wezterm",
];

pub fn enrich_snapshot(mut snapshot: ActivitySnapshot) -> ActivitySnapshot {
    if ZED_BUNDLE_IDS.contains(&snapshot.app_bundle_id.as_str()) {
        if let Some(parsed) = zed::parse_zed_title(&snapshot.window_title) {
            snapshot.context.project = Some(parsed.project);
            snapshot.context.file = Some(parsed.file);
        }
    }

    if TERMINAL_BUNDLE_IDS.contains(&snapshot.app_bundle_id.as_str()) {
        if let Some(hook) = terminal::read_terminal_hook_state() {
            snapshot.context.cwd = hook.cwd;
            snapshot.context.git_branch = hook.git_branch;
        } else if let Some(parsed) = terminal::parse_terminal_title(&snapshot.window_title) {
            snapshot.context.cwd = parsed.cwd;
            snapshot.context.git_branch = parsed.git_branch;
        }
    }

    if let Some(url) = snapshot.context.url.as_ref() {
        snapshot.context.url = browser::normalize_url(url);
    }

    snapshot
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enriches_zed_snapshot() {
        let snapshot = ActivitySnapshot {
            app_name: "Zed".into(),
            app_bundle_id: "dev.zed.Zed".into(),
            window_title: "main.rs — timetrack".into(),
            context: ActivityContext::default(),
            is_idle: false,
        };

        let enriched = enrich_snapshot(snapshot);
        assert_eq!(enriched.context.project.as_deref(), Some("timetrack"));
        assert_eq!(enriched.context.file.as_deref(), Some("main.rs"));
    }

    #[test]
    fn enriches_terminal_from_title() {
        let snapshot = ActivitySnapshot {
            app_name: "Terminal".into(),
            app_bundle_id: "com.apple.Terminal".into(),
            window_title: "daniel@mac:~/code (main)".into(),
            context: ActivityContext::default(),
            is_idle: false,
        };

        let enriched = enrich_snapshot(snapshot);
        assert_eq!(enriched.context.git_branch.as_deref(), Some("main"));
    }

    #[test]
    fn normalizes_browser_url() {
        let snapshot = ActivitySnapshot {
            app_name: "Chrome".into(),
            app_bundle_id: "com.google.Chrome".into(),
            window_title: "GitHub".into(),
            context: ActivityContext {
                url: Some("github.com/user/repo".into()),
                ..Default::default()
            },
            is_idle: false,
        };

        let enriched = enrich_snapshot(snapshot);
        assert_eq!(
            enriched.context.url.as_deref(),
            Some("https://github.com/user/repo")
        );
    }

    #[test]
    fn leaves_unrelated_apps_unchanged() {
        let snapshot = ActivitySnapshot {
            app_name: "Slack".into(),
            app_bundle_id: "com.tinyspeck.slackmacgap".into(),
            window_title: "#general".into(),
            context: ActivityContext::default(),
            is_idle: false,
        };

        let enriched = enrich_snapshot(snapshot);
        assert!(enriched.context.project.is_none());
        assert!(enriched.context.git_branch.is_none());
        assert_eq!(enriched.window_title, "#general");
    }
}
