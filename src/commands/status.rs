use super::*;

pub(super) fn cmd_status(root: &Path, json_mode: bool) -> Result<()> {
    let db = ensure_initialized(root)?;

    let project_name = root
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "project".to_string());

    let total_files = db.count_files()?;
    let total_lines = db.total_lines()?;
    let total_symbols = db.count_symbols()?;
    let total_deps = db.count_dependencies()?;
    let symbol_kinds = db.count_symbols_by_kind()?;
    let lang_stats = db.language_stats()?;
    let decisions = db.get_decisions()?;
    let knowledge = db.get_knowledge()?;

    if json_mode {
        let health = db.get_file_health()?;
        let fragile_count = health.iter().filter(|h| h.is_fragile).count();
        let dead_count = health.iter().filter(|h| h.is_dead).count();

        let kinds: serde_json::Map<String, serde_json::Value> = symbol_kinds
            .iter()
            .map(|(k, v)| (k.clone(), json!(v)))
            .collect();

        let langs: Vec<_> = lang_stats
            .iter()
            .map(|(l, c, lines)| json!({"language": l, "files": c, "lines": lines}))
            .collect();

        println!(
            "{}",
            json!({
                "command": "status",
                "project": project_name,
                "files": total_files,
                "lines": total_lines,
                "symbols": total_symbols,
                "dependencies": total_deps,
                "decisions": decisions.len(),
                "knowledge_notes": knowledge.len(),
                "symbol_kinds": kinds,
                "languages": langs,
                "fragile_files": fragile_count,
                "dead_files": dead_count,
            })
        );
    } else {
        println!(
            "\n  {} {} {}\n",
            "ctx-agent".cyan().bold(),
            "—".dimmed(),
            project_name.white().bold(),
        );

        println!("  Files: {}", total_files.to_string().cyan().bold());
        println!("  Lines: {}", total_lines.to_string().cyan().bold());
        println!("  Symbols: {}", total_symbols.to_string().cyan().bold());
        println!("  Dependencies: {}", total_deps.to_string().cyan().bold());
        println!(
            "  Decisions: {} tracked",
            decisions.len().to_string().cyan().bold()
        );
        println!("  Notes: {}", knowledge.len().to_string().cyan().bold());

        if !symbol_kinds.is_empty() {
            println!("\n  {}", "Symbols:".white().bold());
            for (kind, count) in &symbol_kinds {
                println!("    {:>12}: {}", kind, count.to_string().cyan());
            }
        }

        if !lang_stats.is_empty() {
            println!("\n  {}", "Languages:".white().bold());
            for (lang, count, lines) in &lang_stats {
                println!("    {:>12}: {} files, {} lines", lang.cyan(), count, lines);
            }
        }

        let health = db.get_file_health()?;
        let fragile: Vec<_> = health.iter().filter(|h| h.is_fragile).collect();
        let dead: Vec<_> = health.iter().filter(|h| h.is_dead).collect();

        if !fragile.is_empty() || !dead.is_empty() {
            println!("\n  {}", "Health:".white().bold());
            if !fragile.is_empty() {
                println!(
                    "    {} {} fragile files (high churn + many dependents)",
                    "WARN".yellow(),
                    fragile.len()
                );
            }
            if !dead.is_empty() {
                println!(
                    "    {} {} potentially dead files (no commits, no dependents)",
                    "WARN".dimmed(),
                    dead.len()
                );
            }
        }

        println!();
    }

    Ok(())
}
