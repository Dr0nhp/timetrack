use std::ffi::CStr;
use std::os::raw::c_char;
use std::path::{Path, PathBuf};

use serde::Deserialize;

const MAX_PID_PATH: usize = 4096;

/// Known window owner names from CGWindowList when executable path lookup fails.
const KNOWN_OWNER_BUNDLES: &[(&str, &str)] = &[
    ("Safari", "com.apple.Safari"),
    ("Google Chrome", "com.google.Chrome"),
    ("Firefox", "org.mozilla.firefox"),
    ("Brave Browser", "com.brave.Browser"),
    ("Microsoft Edge", "com.microsoft.edgemac"),
    ("Arc", "company.thebrowser.Browser"),
    ("Opera", "com.operasoftware.Opera"),
    ("Vivaldi", "com.vivaldi.Vivaldi"),
    ("Terminal", "com.apple.Terminal"),
    ("iTerm2", "com.googlecode.iterm2"),
    ("Zed", "dev.zed.Zed"),
    ("Cursor", "com.todesktop.230313mzl4w4u92"),
    ("Telegram", "ru.keepcoder.Telegram"),
    ("Slack", "com.tinyspeck.slackmacgap"),
    ("Code", "com.microsoft.VSCode"),
    ("TimeTrack", "com.timetrack.app"),
    ("timetrack", "com.timetrack.app"),
];

#[link(name = "proc")]
extern "C" {
    fn proc_pidpath(pid: i32, buffer: *mut c_char, buffer_size: u32) -> i32;
}

#[derive(Debug, Deserialize)]
struct InfoPlist {
    #[serde(rename = "CFBundleIdentifier")]
    bundle_id: String,
}

/// Resolve bundle ID without AppKit — safe on background threads.
pub fn bundle_id_for_pid(pid: i32, owner_name: &str) -> String {
    bundle_id_from_executable(pid)
        .or_else(|| bundle_id_from_owner_name(owner_name))
        .unwrap_or_default()
}

fn bundle_id_from_owner_name(owner_name: &str) -> Option<String> {
    KNOWN_OWNER_BUNDLES
        .iter()
        .find(|(name, _)| *name == owner_name)
        .map(|(_, bundle)| (*bundle).to_string())
}

fn bundle_id_from_executable(pid: i32) -> Option<String> {
    let executable = executable_path_for_pid(pid)?;
    let app_bundle = app_bundle_path(&executable)?;
    let plist_path = app_bundle.join("Contents/Info.plist");
    read_bundle_id_from_plist(&plist_path)
}

fn executable_path_for_pid(pid: i32) -> Option<PathBuf> {
    let mut buffer = vec![0u8; MAX_PID_PATH];
    let len = unsafe {
        proc_pidpath(
            pid,
            buffer.as_mut_ptr() as *mut c_char,
            buffer.len() as u32,
        )
    };
    if len <= 0 {
        return None;
    }

    let path = CStr::from_bytes_until_nul(&buffer[..len as usize])
        .ok()?
        .to_str()
        .ok()?;
    Some(PathBuf::from(path))
}

fn app_bundle_path(executable: &Path) -> Option<PathBuf> {
    for ancestor in executable.ancestors() {
        if ancestor.extension().is_some_and(|ext| ext == "app") {
            return Some(ancestor.to_path_buf());
        }
    }
    None
}

fn read_bundle_id_from_plist(path: &Path) -> Option<String> {
    let file = std::fs::File::open(path).ok()?;
    let plist: InfoPlist = plist::from_reader(file).ok()?;
    if plist.bundle_id.is_empty() {
        None
    } else {
        Some(plist.bundle_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_known_owner_names() {
        assert_eq!(
            bundle_id_from_owner_name("Safari"),
            Some("com.apple.Safari".into())
        );
    }

    #[test]
    fn finds_app_bundle_in_path() {
        let path = PathBuf::from("/Applications/Safari.app/Contents/MacOS/Safari");
        assert_eq!(
            app_bundle_path(&path),
            Some(PathBuf::from("/Applications/Safari.app"))
        );
    }
}
