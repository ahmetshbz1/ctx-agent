use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use colored::*;
use serde_json::json;
use std::path::PathBuf;
use std::time::Instant;

use ctx::db::Database;
use ctx::analyzer;
use ctx::git;
use ctx::query;
use ctx::watcher;

#[derive(Parser)]
#[command(
    name = "ctx-agent",
    version,
    about = "Agent Context Protocol — Structured codebase intelligence for AI agents",
    long_about = "Live codebase intelligence for AI agents. Zero dependencies, local-first, offline-capable."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Project root directory (defaults to current directory)
    #[arg(short, long, global = true)]
    project: Option<PathBuf>,

    /// Output in JSON format (for agent consumption)
    #[arg(long, global = true)]
    json: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize ctx-agent in the current project
    Init,

    /// Scan/re-scan the project
    Scan,

    /// Display codebase map with structure and stats
    Map,

    /// Show project status dashboard
    Status,

    /// Search symbols and files
    Query {
        /// Search term
        term: String,
    },

    /// Show blast radius of changing a file
    BlastRadius {
        /// File path (relative to project root)
        path: String,
    },

    /// Show recorded decisions
    Decisions,

    /// Add a knowledge note
    Learn {
        /// Note content
        note: String,

        /// Related file (optional)
        #[arg(short, long)]
        file: Option<String>,
    },

    /// Show warnings (fragile files, dead code)
    Warnings,

    /// Watch for file changes and re-analyze
    Watch,
}

fn get_project_root(cli: &Cli) -> Result<PathBuf> {
    match &cli.project {
        Some(p) => Ok(p.clone()),
        None => std::env::current_dir().context("Failed to get current directory"),
    }
}

fn ensure_initialized(root: &PathBuf) -> Result<Database> {
    if !Database::exists(root) {
        anyhow::bail!(
            "ctx-agent is not initialized in this project.\nRun {} first.",
            "ctx-agent init".cyan()
        );
    }
    Database::open(root)
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let root = get_project_root(&cli)?;
    let json_mode = cli.json;

    match cli.command {
        Commands::Init => cmd_init(&root, json_mode)?,
        Commands::Scan => cmd_scan(&root, json_mode)?,
        Commands::Map => cmd_map(&root, json_mode)?,
        Commands::Status => cmd_status(&root, json_mode)?,
        Commands::Query { term } => cmd_query(&root, &term, json_mode)?,
        Commands::BlastRadius { path } => cmd_blast_radius(&root, &path, json_mode)?,
        Commands::Decisions => cmd_decisions(&root, json_mode)?,
        Commands::Learn { note, file } => cmd_learn(&root, &note, file.as_deref(), json_mode)?,
        Commands::Warnings => cmd_warnings(&root, json_mode)?,
        Commands::Watch => cmd_watch(&root)?,
    }

    Ok(())
}

// =================================================================
// Command implementations
// =================================================================

fn cmd_init(root: &PathBuf, json_mode: bool) -> Result<()> {
    if !json_mode {
        println!("\n  {} {}\n", "ctx-agent".cyan().bold(), "— Agent Context Protocol");
    }

    if Database::exists(root) {
        if !json_mode {
            println!("  {} Already initialized. Running re-scan...\n", "⚡".yellow());
        }
        return cmd_scan(root, json_mode);
    }

    let start = Instant::now();
    let db = Database::open(root)?;

    if !json_mode {
        println!("  {} Created {}", "✓".green(), ".ctx/ctx.db".dimmed());
        print!("  ⟳ Scanning project...");
    }

    let result = analyzer::analyze_project(&db, root)?;

    if !json_mode {
        println!(" {}", "done".green());
        println!("    {} files discovered", result.total_files.to_string().cyan());
        println!("    {} symbols extracted", result.total_symbols.to_string().cyan());
        println!("    {} dependencies mapped", result.total_imports.to_string().cyan());
        print!("  ⟳ Analyzing git history...");
    }

    let git_result = git::analyze_git_history(&db, root)?;

    if json_mode {
        let elapsed = start.elapsed();
        println!("{}", json!({
            "command": "init",
            "files": result.total_files,
            "symbols": result.total_symbols,
            "dependencies": result.total_imports,
            "commits_analyzed": git_result.commits_analyzed,
            "decisions_found": git_result.decisions_found,
            "elapsed_ms": elapsed.as_millis(),
        }));
    } else {
        println!(" {}", "done".green());
        if let Some(err) = &git_result.error {
            println!("    {} {}", "⚠".yellow(), err.dimmed());
        } else {
            println!("    {} commits analyzed", git_result.commits_analyzed.to_string().cyan());
            println!("    {} decisions extracted", git_result.decisions_found.to_string().cyan());
        }

        let elapsed = start.elapsed();
        println!("\n  {} Initialized in {:.1}s\n", "✓".green().bold(), elapsed.as_secs_f64());
    }

    // Add .ctx to .gitignore if not already there
    let gitignore_path = root.join(".gitignore");
    if gitignore_path.exists() {
        let content = std::fs::read_to_string(&gitignore_path).unwrap_or_default();
        if !content.contains(".ctx") {
            let mut new_content = content;
            if !new_content.ends_with('\n') {
                new_content.push('\n');
            }
            new_content.push_str("\n# ctx context database\n.ctx/\n");
            std::fs::write(&gitignore_path, new_content)?;
            if !json_mode {
                println!("  {} Added .ctx/ to .gitignore", "✓".green());
            }
        }
    }

    Ok(())
}

fn cmd_scan(root: &PathBuf, json_mode: bool) -> Result<()> {
    let db = ensure_initialized(root)?;
    let start = Instant::now();

    if !json_mode {
        print!("  ⟳ Scanning...");
    }

    let result = analyzer::analyze_project(&db, root)?;
    let git_result = git::analyze_git_history(&db, root)?;
    let elapsed = start.elapsed();

    if json_mode {
        println!("{}", json!({
            "command": "scan",
            "total_files": result.total_files,
            "analyzed_files": result.analyzed_files,
            "skipped_files": result.skipped_files,
            "removed_files": result.removed_files,
            "symbols": result.total_symbols,
            "dependencies": result.total_imports,
            "commits_analyzed": git_result.commits_analyzed,
            "elapsed_ms": elapsed.as_millis(),
        }));
    } else {
        println!(" {}", "done".green());
        println!("    {} files ({} analyzed, {} unchanged, {} removed)",
            result.total_files.to_string().cyan(),
            result.analyzed_files.to_string().green(),
            result.skipped_files.to_string().dimmed(),
            result.removed_files.to_string().red(),
        );
        println!("    {} symbols, {} dependencies",
            result.total_symbols.to_string().cyan(),
            result.total_imports.to_string().cyan(),
        );
        if git_result.error.is_none() {
            println!("    {} git commits analyzed", git_result.commits_analyzed.to_string().cyan());
        }
        println!("  {} Completed in {:.1}s\n", "✓".green(), elapsed.as_secs_f64());
    }

    Ok(())
}

fn cmd_map(root: &PathBuf, json_mode: bool) -> Result<()> {
    let db = ensure_initialized(root)?;
    let files = db.get_all_files()?;

    if files.is_empty() {
        if json_mode {
            println!("{}", json!({ "command": "map", "directories": [] }));
        } else {
            println!("  {} No files found. Run {} first.", "!".yellow(), "ctx scan".cyan());
        }
        return Ok(());
    }

    // Group files by directory
    let mut dir_map: std::collections::BTreeMap<String, Vec<&ctx::db::models::TrackedFile>> = std::collections::BTreeMap::new();
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
            let languages: std::collections::HashSet<&str> = dir_files.iter().map(|f| f.language.as_str()).collect();

            let mut file_entries = Vec::new();
            for file in dir_files {
                let symbols = db.get_symbols_for_file(file.id)?;
                let sym_names: Vec<_> = symbols.iter()
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
        let project_name = root.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "project".to_string());

        println!("\n  {} {}\n", "📦".to_string(), project_name.cyan().bold());

        for (dir, dir_files) in &dir_map {
            let total_lines: i64 = dir_files.iter().map(|f| f.line_count).sum();
            let languages: std::collections::HashSet<&str> = dir_files.iter().map(|f| f.language.as_str()).collect();
            let lang_str = languages.into_iter().collect::<Vec<_>>().join(", ");

            println!("  {} {}  {} files, {} lines  ({})",
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
                    println!("  {}   {} {}", "│".dimmed(), "·".dimmed(), file_name.dimmed());
                } else {
                    let sym_summary: Vec<String> = symbols.iter()
                        .filter(|s| s.parent_symbol_id.is_none())
                        .take(5)
                        .map(|s| format!("{} {}", s.kind.icon().dimmed(), s.name))
                        .collect();
                    let remaining = symbols.iter().filter(|s| s.parent_symbol_id.is_none()).count().saturating_sub(5);
                    let extra = if remaining > 0 { format!(" +{}", remaining) } else { String::new() };

                    println!("  {}   {} {} → {}{}",
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
            println!("  {:>12}  {} {} files, {} lines", lang.cyan(), bar.green(), count, lines);
        }
        println!();
    }

    Ok(())
}

fn cmd_status(root: &PathBuf, json_mode: bool) -> Result<()> {
    let db = ensure_initialized(root)?;

    let project_name = root.file_name()
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

        let kinds: serde_json::Map<String, serde_json::Value> = symbol_kinds.iter()
            .map(|(k, v)| (k.clone(), json!(v)))
            .collect();

        let langs: Vec<_> = lang_stats.iter()
            .map(|(l, c, lines)| json!({"language": l, "files": c, "lines": lines}))
            .collect();

        println!("{}", json!({
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
        }));
    } else {
        println!("\n  {} {} {}\n",
            "ctx-agent".cyan().bold(),
            "—".dimmed(),
            project_name.white().bold(),
        );

        println!("  {}  {} files", "📄", total_files.to_string().cyan().bold());
        println!("  {}  {} lines of code", "📝", total_lines.to_string().cyan().bold());
        println!("  {}  {} symbols", "🔣", total_symbols.to_string().cyan().bold());
        println!("  {}  {} dependencies", "🔗", total_deps.to_string().cyan().bold());
        println!("  {}  {} decisions tracked", "📋", decisions.len().to_string().cyan().bold());
        println!("  {}  {} knowledge notes", "🧠", knowledge.len().to_string().cyan().bold());

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
                println!("    {} {} fragile files (high churn + many dependents)", "⚠".yellow(), fragile.len());
            }
            if !dead.is_empty() {
                println!("    {} {} potentially dead files (no commits, no dependents)", "💀".to_string().dimmed(), dead.len());
            }
        }

        println!();
    }

    Ok(())
}

fn cmd_query(root: &PathBuf, term: &str, json_mode: bool) -> Result<()> {
    let db = ensure_initialized(root)?;

    if json_mode {
        let results = db.search(term)?;
        let entries: Vec<_> = results.iter().map(|(name, path, kind, signature)| json!({
            "name": name,
            "kind": kind,
            "signature": signature,
            "file": path,
        })).collect();
        println!("{}", json!({
            "command": "query",
            "term": term,
            "count": entries.len(),
            "results": entries,
        }));
    } else {
        println!();
        query::execute_search(&db, term)?;
        println!();
    }

    Ok(())
}

fn cmd_blast_radius(root: &PathBuf, path: &str, json_mode: bool) -> Result<()> {
    let db = ensure_initialized(root)?;

    if json_mode {
        let file_id = match db.get_file_id(path)? {
            Some(id) => id,
            None => {
                println!("{}", json!({
                    "command": "blast_radius",
                    "error": format!("File not found: {}", path),
                }));
                return Ok(());
            }
        };

        let deps = db.get_dependencies_of(file_id)?;
        let dependents = db.get_dependents(file_id)?;

        let dep_list: Vec<_> = deps.iter().map(|(_, dep_path)| json!({
            "target": dep_path,
        })).collect();

        let dep_of_list: Vec<_> = dependents.iter().map(|(_, dep_path)| json!({
            "source": dep_path,
        })).collect();

        // Transitive blast radius
        let radius = ctx::analyzer::graph::blast_radius(&db, file_id)?;
        let radius_list: Vec<_> = radius.iter().map(|(_, rpath, depth)| json!({
            "path": rpath,
            "depth": depth,
        })).collect();

        let risk = match radius.len() {
            0 => "low",
            1..=5 => "medium",
            6..=20 => "high",
            _ => "critical",
        };

        println!("{}", json!({
            "command": "blast_radius",
            "file": path,
            "imports": dep_list,
            "imported_by": dep_of_list,
            "transitive_impact": radius_list,
            "risk": risk,
        }));
    } else {
        query::execute_blast_radius(&db, path)?;
        println!();
    }

    Ok(())
}

fn cmd_decisions(root: &PathBuf, json_mode: bool) -> Result<()> {
    let db = ensure_initialized(root)?;
    let decisions = db.get_decisions()?;

    if json_mode {
        let entries: Vec<_> = decisions.iter().map(|d| json!({
            "timestamp": d.timestamp,
            "source": d.source,
            "description": d.description,
            "commit_hash": d.commit_hash,
        })).collect();
        println!("{}", json!({
            "command": "decisions",
            "count": entries.len(),
            "decisions": entries,
        }));
    } else {
        if decisions.is_empty() {
            println!("\n  {} No decisions recorded yet.", "·".dimmed());
            println!("  Decisions are extracted from git commits (feat:, refactor:, BREAKING, etc.)");
            println!("  or added manually with: {}\n", "ctx-agent learn \"decision description\"".cyan());
            return Ok(());
        }

        println!("\n  {} {} decisions\n", "📋", decisions.len().to_string().cyan().bold());

        for decision in decisions.iter().take(30) {
            let source_badge = match decision.source.as_str() {
                "commit" => "commit".blue(),
                "manual" => "manual".green(),
                _ => decision.source.as_str().into(),
            };
            let hash = decision.commit_hash.as_deref().unwrap_or("").chars().take(8).collect::<String>();
            let hash_str = if !hash.is_empty() { format!(" ({})", hash).dimmed().to_string() } else { String::new() };

            println!("  {} [{}] {}{}",
                decision.timestamp.get(..10).unwrap_or(&decision.timestamp).dimmed(),
                source_badge,
                decision.description.lines().next().unwrap_or(""),
                hash_str,
            );
        }

        if decisions.len() > 30 {
            println!("\n  {} ... and {} more", "·".dimmed(), decisions.len() - 30);
        }

        println!();
    }

    Ok(())
}

fn cmd_learn(root: &PathBuf, note: &str, file: Option<&str>, json_mode: bool) -> Result<()> {
    let db = ensure_initialized(root)?;
    db.insert_knowledge(note, "manual", file)?;

    if json_mode {
        println!("{}", json!({
            "command": "learn",
            "note": note,
            "file": file,
            "status": "recorded",
        }));
    } else {
        println!("\n  {} Knowledge recorded", "✓".green().bold());
        if let Some(f) = file {
            println!("  Related file: {}", f.cyan());
        }
        println!("  \"{}\"", note.white());
        println!();
    }

    Ok(())
}

fn cmd_warnings(root: &PathBuf, json_mode: bool) -> Result<()> {
    let db = ensure_initialized(root)?;
    let health = db.get_file_health()?;
    let knowledge = db.get_warnings_knowledge()?;

    let fragile: Vec<_> = health.iter().filter(|h| h.is_fragile).collect();
    let dead: Vec<_> = health.iter().filter(|h| h.is_dead).collect();
    let large: Vec<_> = health.iter().filter(|h| h.line_count > 500).collect();

    if json_mode {
        let fragile_entries: Vec<_> = fragile.iter().map(|f| json!({
            "path": f.path,
            "commit_count": f.commit_count,
            "dependents": f.dependents_count,
            "churn_score": f.churn_score,
        })).collect();

        let large_entries: Vec<_> = large.iter().map(|f| json!({
            "path": f.path,
            "lines": f.line_count,
            "language": f.language,
        })).collect();

        let dead_entries: Vec<_> = dead.iter().map(|f| json!({
            "path": f.path,
            "language": f.language,
        })).collect();

        let knowledge_entries: Vec<_> = knowledge.iter().map(|k| json!({
            "content": k.content,
            "file": k.related_file,
        })).collect();

        println!("{}", json!({
            "command": "warnings",
            "total_warnings": fragile.len() + dead.len() + large.len() + knowledge.len(),
            "fragile_files": fragile_entries,
            "large_files": large_entries,
            "dead_files": dead_entries,
            "knowledge_warnings": knowledge_entries,
        }));
    } else {
        let total_warnings = fragile.len() + dead.len() + large.len() + knowledge.len();

        if total_warnings == 0 {
            println!("\n  {} No warnings — looking good!\n", "✓".green().bold());
            return Ok(());
        }

        println!("\n  {} {} warnings\n", "⚠".yellow().bold(), total_warnings.to_string().yellow().bold());

        if !fragile.is_empty() {
            println!("  {} Fragile files (high churn + many dependents):", "🔥".to_string());
            for f in &fragile {
                println!("    {} {} — {} commits, {} dependents, churn {:.1}",
                    "⚠".yellow(),
                    f.path.red(),
                    f.commit_count.to_string().cyan(),
                    f.dependents_count.to_string().cyan(),
                    f.churn_score,
                );
            }
            println!();
        }

        if !large.is_empty() {
            println!("  {} Large files (>500 lines):", "📏".to_string());
            for f in &large {
                println!("    {} {} — {} lines ({})",
                    "·".dimmed(),
                    f.path.yellow(),
                    f.line_count.to_string().cyan(),
                    f.language.dimmed(),
                );
            }
            println!();
        }

        if !dead.is_empty() {
            println!("  {} Potentially dead files (no git history, no dependents):", "💀".to_string());
            for f in dead.iter().take(10) {
                println!("    {} {} ({})", "·".dimmed(), f.path.dimmed(), f.language.dimmed());
            }
            if dead.len() > 10 {
                println!("    {} ... and {} more", "·".dimmed(), dead.len() - 10);
            }
            println!();
        }

        if !knowledge.is_empty() {
            println!("  {} Agent-discovered issues:", "🧠".to_string());
            for k in &knowledge {
                let file_str = k.related_file.as_deref().unwrap_or("");
                println!("    {} {} {}",
                    "⚠".yellow(),
                    k.content,
                    if !file_str.is_empty() { format!("({})", file_str).dimmed().to_string() } else { String::new() },
                );
            }
            println!();
        }
    }

    Ok(())
}

fn cmd_watch(root: &PathBuf) -> Result<()> {
    let db = ensure_initialized(root)?;
    drop(db); // Close db before watcher opens its own

    println!("\n  {} {}\n", "ctx-agent".cyan().bold(), "— Watch Mode");
    watcher::watch_project(root)?;
    Ok(())
}
