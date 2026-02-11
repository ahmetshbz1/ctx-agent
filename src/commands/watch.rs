use super::*;

pub(super) fn cmd_watch(root: &Path) -> Result<()> {
    let db = ensure_initialized(root)?;
    drop(db); // Close db before watcher opens its own

    println!("\n  {} — Watch Mode\n", "ctx-agent".cyan().bold());
    watcher::watch_project(root)?;
    Ok(())
}
