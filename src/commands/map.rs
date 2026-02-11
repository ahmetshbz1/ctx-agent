use super::*;

pub(super) fn cmd_map(root: &Path, json_mode: bool) -> Result<()> {
    let db = ensure_initialized(root)?;
    let files = db.get_all_files()?;

    if files.is_empty() {
        if json_mode {
            println!("{}", json!({ "command": "map", "directories": [] }));
        } else {
            println!(
                "  {} No files found. Run {} first.",
                "!".yellow(),
                "ctx scan".cyan()
            );
        }
        return Ok(());
    }

    // Group files by directory
    let mut dir_map: std::collections::BTreeMap<String, Vec<&ctx::db::models::TrackedFile>> =
        std::collections::BTreeMap::new();
    for file in &files {
        let dir = std::path::Path::new(&file.path)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| ".".to_string());
        dir_map.entry(dir).or_default().push(file);
    }

    if json_mode {
        let mut dirs = Vec::new();
        for (dir, dir_files) in &dir_map {
            let total_lines: i64 = dir_files.iter().map(|f| f.line_count).sum();
            let languages: std::collections::HashSet<&str> =
                dir_files.iter().map(|f| f.language.as_str()).collect();

            let mut file_entries = Vec::new();
            for file in dir_files {
                let symbols = db.get_symbols_for_file(file.id)?;
                let sym_names: Vec<_> = symbols
                    .iter()
                    .filter(|s| s.parent_symbol_id.is_none())
                    .map(|s| json!({"name": s.name, "kind": format!("{:?}", s.kind)}))
                    .collect();
                file_entries.push(json!({
                    "path": file.path,
                    "language": file.language,
                    "lines": file.line_count,
                    "symbols": sym_names,
                }));
            }

            dirs.push(json!({
                "directory": dir,
                "files_count": dir_files.len(),
                "total_lines": total_lines,
                "languages": languages.into_iter().collect::<Vec<_>>(),
                "files": file_entries,
            }));
        }
        println!("{}", json!({ "command": "map", "directories": dirs }));
    } else {
        let project_name = root
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "project".to_string());

        println!("\n  📦 {}\n", project_name.cyan().bold());

        for (dir, dir_files) in &dir_map {
            let total_lines: i64 = dir_files.iter().map(|f| f.line_count).sum();
            let languages: std::collections::HashSet<&str> =
                dir_files.iter().map(|f| f.language.as_str()).collect();
            let lang_str = languages.into_iter().collect::<Vec<_>>().join(", ");

            println!(
                "  {} {}  {} files, {} lines  ({})",
                "├──".dimmed(),
                dir.white().bold(),
                dir_files.len().to_string().cyan(),
                total_lines.to_string().cyan(),
                lang_str.dimmed(),
            );

            for file in dir_files {
                let file_name = std::path::Path::new(&file.path)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| file.path.clone());

                let symbols = db.get_symbols_for_file(file.id)?;
                if symbols.is_empty() {
                    println!(
                        "  {}   {} {}",
                        "│".dimmed(),
                        "·".dimmed(),
                        file_name.dimmed()
                    );
                } else {
                    let sym_summary: Vec<String> = symbols
                        .iter()
                        .filter(|s| s.parent_symbol_id.is_none())
                        .take(5)
                        .map(|s| format!("{} {}", s.kind.icon().dimmed(), s.name))
                        .collect();
                    let remaining = symbols
                        .iter()
                        .filter(|s| s.parent_symbol_id.is_none())
                        .count()
                        .saturating_sub(5);
                    let extra = if remaining > 0 {
                        format!(" +{}", remaining)
                    } else {
                        String::new()
                    };

                    println!(
                        "  {}   {} {} → {}{}",
                        "│".dimmed(),
                        "·".dimmed(),
                        file_name,
                        sym_summary.join(", ").dimmed(),
                        extra.dimmed(),
                    );
                }
            }
            println!("  {}", "│".dimmed());
        }

        let lang_stats = db.language_stats()?;
        println!("\n  {}", "Languages:".white().bold());
        for (lang, count, lines) in &lang_stats {
            let bar_len = (*lines as f64 / lang_stats[0].2 as f64 * 20.0) as usize;
            let bar = "█".repeat(bar_len);
            println!(
                "  {:>12}  {} {} files, {} lines",
                lang.cyan(),
                bar.green(),
                count,
                lines
            );
        }
        println!();
    }

    Ok(())
}
