//! Telemetry guard — H-M4, structural.
//!
//! The native side carries no usable analytics endpoint URL. The guard
//! is on the full URL (a network destination), not the bare domain —
//! the domain is named in the audit documentation deliberately, as the
//! abandoned trigger. The guard lives in this file (not in `src/`):
//! the guard literal must not sit in a file the guard itself scans,
//! or the scan would match its own check.

#[test]
fn no_telemetry_endpoint_in_the_adapter() {
    let dir = format!("{}/src", env!("CARGO_MANIFEST_DIR"));
    let mut found: Vec<String> = Vec::new();
    scan(&dir, &mut found);
    assert!(
        found.is_empty(),
        "telemetry endpoint URL found in: {found:?}"
    );
}

fn scan(dir: &str, found: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            scan(&path.to_string_lossy(), found);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            if let Ok(text) = std::fs::read_to_string(&path) {
                if text.contains("https://api.primeintellect.ai") {
                    found.push(path.to_string_lossy().into_owned());
                }
            }
        }
    }
}
