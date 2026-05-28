use std::fs;
use std::path::PathBuf;

use regex::Regex;
use serde::Deserialize;
use std::sync::LazyLock;

static BRANCH_PARENS_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\(([^)]+)\)\s*$").expect("valid branch parens regex"));
static BRANCH_BRACKETS_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\[([^\]]+)\]").expect("valid branch brackets regex"));
static STARSHIP_GIT_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"git:(\S+)").expect("valid starship git regex"));

#[derive(Debug, Clone)]
pub struct TerminalContext {
    pub cwd: Option<String>,
    pub git_branch: Option<String>,
}

#[derive(Debug, Deserialize)]
struct HookLine {
    cwd: Option<String>,
    branch: Option<String>,
}

pub fn parse_terminal_title(title: &str) -> Option<TerminalContext> {
    let git_branch = parse_branch_from_title(title);
    if git_branch.is_none() {
        return None;
    }

    Some(TerminalContext {
        cwd: parse_cwd_from_title(title),
        git_branch,
    })
}

fn parse_branch_from_title(title: &str) -> Option<String> {
    if let Some(caps) = STARSHIP_GIT_RE.captures(title) {
        return caps.get(1).map(|m| m.as_str().to_string());
    }
    if let Some(caps) = BRANCH_BRACKETS_RE.captures(title) {
        let branch = caps.get(1)?.as_str();
        if looks_like_branch(branch) {
            return Some(branch.to_string());
        }
    }
    if let Some(caps) = BRANCH_PARENS_RE.captures(title) {
        let branch = caps.get(1)?.as_str();
        if looks_like_branch(branch) {
            return Some(branch.to_string());
        }
    }
    None
}

fn looks_like_branch(value: &str) -> bool {
    !value.is_empty()
        && !value.contains('@')
        && !value.contains(':')
        && value.len() < 120
}

fn parse_cwd_from_title(title: &str) -> Option<String> {
    if title.starts_with("~/") || title.starts_with('/') {
        return Some(title.to_string());
    }

    if let Some(idx) = title.find(':') {
        let tail = &title[idx + 1..];
        let path = tail.split([' ', '(', '[']).next()?.trim();
        if path.starts_with('/') || path.starts_with('~') {
            return Some(path.to_string());
        }
    }

    None
}

pub fn read_terminal_hook_state() -> Option<TerminalContext> {
    let path = hook_state_path()?;
    read_terminal_hook_state_from(path)
}

pub fn read_terminal_hook_state_from(path: impl AsRef<std::path::Path>) -> Option<TerminalContext> {
    let content = fs::read_to_string(path).ok()?;
    let line = content.lines().rev().find(|l| !l.trim().is_empty())?;
    let parsed: HookLine = serde_json::from_str(line).ok()?;

    Some(TerminalContext {
        cwd: parsed.cwd,
        git_branch: parsed.branch,
    })
}

pub fn hook_state_path() -> Option<PathBuf> {
    dirs::home_dir().map(|home| home.join(".timetrack").join("terminal-state.jsonl"))
}

pub fn hook_install_script() -> String {
    r#"# TimeTrack terminal hook — add to ~/.zshrc or ~/.bashrc
_timetrack_hook() {
  local cwd branch
  cwd=$(pwd)
  branch=$(git branch --show-current 2>/dev/null)
  mkdir -p "$HOME/.timetrack"
  printf '{"cwd":"%s","branch":"%s","ts":%s}\n' \
    "$cwd" "${branch:-}" "$(date +%s)" >> "$HOME/.timetrack/terminal-state.jsonl"
}
# zsh
precmd_functions+=(_timetrack_hook)
# bash: PROMPT_COMMAND="_timetrack_hook; $PROMPT_COMMAND"
"#
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn parses_branch_in_parens() {
        let ctx = parse_terminal_title("daniel@mac:~/projects/timetrack (feature/auth)")
            .unwrap();
        assert_eq!(ctx.git_branch.as_deref(), Some("feature/auth"));
        assert_eq!(ctx.cwd.as_deref(), Some("~/projects/timetrack"));
    }

    #[test]
    fn parses_starship_git() {
        let ctx = parse_terminal_title("~/projects/timetrack main git:feature/auth").unwrap();
        assert_eq!(ctx.git_branch.as_deref(), Some("feature/auth"));
        assert_eq!(ctx.cwd.as_deref(), Some("~/projects/timetrack main git:feature/auth"));
    }

    #[test]
    fn parses_branch_in_brackets() {
        let ctx = parse_terminal_title("timetrack [main]").unwrap();
        assert_eq!(ctx.git_branch.as_deref(), Some("main"));
    }

    #[test]
    fn rejects_title_without_branch() {
        assert!(parse_terminal_title("daniel@mac:~/projects/timetrack").is_none());
    }

    #[test]
    fn rejects_host_like_parens() {
        assert!(parse_terminal_title("shell (daniel@mac)").is_none());
    }

    #[test]
    fn reads_latest_hook_line() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("terminal-state.jsonl");
        fs::write(
            &path,
            r#"{"cwd":"/projects/a","branch":"main","ts":1}
{"cwd":"/projects/b","branch":"feature/auth","ts":2}
"#,
        )
        .unwrap();

        let ctx = read_terminal_hook_state_from(&path).unwrap();
        assert_eq!(ctx.cwd.as_deref(), Some("/projects/b"));
        assert_eq!(ctx.git_branch.as_deref(), Some("feature/auth"));
    }

    #[test]
    fn hook_reader_skips_blank_lines_at_end() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("terminal-state.jsonl");
        let mut file = fs::File::create(&path).unwrap();
        writeln!(
            file,
            r#"{{"cwd":"/x","branch":"dev","ts":3}}"#
        )
        .unwrap();
        writeln!(file).unwrap();

        let ctx = read_terminal_hook_state_from(&path).unwrap();
        assert_eq!(ctx.git_branch.as_deref(), Some("dev"));
    }

    #[test]
    fn hook_install_script_contains_precmd() {
        let script = hook_install_script();
        assert!(script.contains("precmd_functions"));
        assert!(script.contains("terminal-state.jsonl"));
    }
}
