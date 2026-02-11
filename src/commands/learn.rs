use super::*;

pub(super) fn cmd_learn(
    root: &Path,
    note: &str,
    file: Option<&str>,
    json_mode: bool,
) -> Result<()> {
    let db = ensure_initialized(root)?;
    db.insert_knowledge(note, "manual", file)?;

    if json_mode {
        println!(
            "{}",
            json!({
                "command": "learn",
                "note": note,
                "file": file,
                "status": "recorded",
            })
        );
    } else {
        println!("\n  {} Knowledge recorded", "OK".green().bold());
        if let Some(f) = file {
            println!("  Related file: {}", f.cyan());
        }
        println!("  \"{}\"", note.white());
        println!();
    }

    Ok(())
}
