use anyhow::Result;
use notify::{Watcher, RecursiveMode, Event, EventKind};
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;

use crate::db::Database;
use crate::analyzer;

/// Start watching for file changes and re-analyze incrementally
pub fn watch_project(project_root: &Path) -> Result<()> {
    let (tx, rx) = mpsc::channel();

    let mut watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
        if let Ok(event) = res {
            tx.send(event).ok();
        }
    })?;

    // Watch the project root (excluding .ctx and .git)
    watcher.watch(project_root, RecursiveMode::Recursive)?;

    println!("  👁  Watching for changes... (Ctrl+C to stop)");

    let db = Database::open(project_root)?;
    let mut debounce_timer = std::time::Instant::now();

    loop {
        match rx.recv_timeout(Duration::from_millis(500)) {
            Ok(event) => {
                // Skip events in .ctx, .git, target directories
                let dominated_by_ignored = event.paths.iter().all(|p| {
                    let path_str = p.to_string_lossy();
                    path_str.contains("/.ctx/") ||
                    path_str.contains("/.git/") ||
                    path_str.contains("/target/") ||
                    path_str.contains("/node_modules/")
                });

                if dominated_by_ignored {
                    continue;
                }

                match event.kind {
                    EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) => {
                        // Debounce: wait at least 1 second between re-analyses
                        if debounce_timer.elapsed() > Duration::from_secs(1) {
                            println!("  ⟳  Change detected, re-analyzing...");
                            match analyzer::analyze_project(&db, project_root) {
                                Ok(result) => {
                                    println!("  ✓  Updated: {} files, {} symbols",
                                        result.analyzed_files, result.total_symbols);
                                }
                                Err(e) => {
                                    eprintln!("  ✗  Analysis error: {}", e);
                                }
                            }
                            debounce_timer = std::time::Instant::now();
                        }
                    }
                    _ => {}
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }

    Ok(())
}
