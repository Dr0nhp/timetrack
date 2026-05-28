fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--diag") {
        run_diag();
        return;
    }
    timetrack_lib::run();
}

fn run_diag() {
    let exe = std::env::current_exe()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "?".into());
    let trusted = timetrack_monitor::is_accessibility_trusted();
    let snap = timetrack_monitor::capture_snapshot();
    println!("binary={exe}");
    println!("accessibility_granted={trusted}");
    match snap {
        Ok(s) => println!(
            "app={} bundle={} title={:?} url={:?}",
            s.app_name, s.app_bundle_id, s.window_title, s.url
        ),
        Err(e) => println!("capture_error={e}"),
    }
    if !trusted {
        eprintln!("\n→ Bedienungshilfen: Systemeinstellungen → Datenschutz → Bedienungshilfen");
        eprintln!("→ Diese Binary aktivieren: {exe}");
    }
}
