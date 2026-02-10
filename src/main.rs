use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use colored::*;
use std::path::PathBuf;
use std::time::Instant;

use ctx::db::Database;
use ctx::analyzer;
use ctx::git;
use ctx::query;
use ctx::watcher;

#[derive(Parser)]
#[command(
    name = "ctx",
    version,
    about = "Universal AI Agent Context Protocol",
    long_about = "Live codebase intelligence for AI agents. Zero dependencies, local-first, offline-capable."
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Project root directory (defaults to current directory)
    #[arg(short, long, global = true)]
    project: Option<PathBuf>,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize ctx in the current project
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
            "ctx is not initialized in this project.\nRun {} first.",
            "ctx init".cyan()
        );
    }
    Database::open(root)
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let root = get_project_root(&cli)?;

    match cli.command {
        Commands::Init => cmd_init(&root)?,
        Commands::Scan => cmd_scan(&root)?,
        Commands::Map => cmd_map(&root)?,
        Commands::Status => cmd_status(&root)?,
        Commands::Query { term } => cmd_query(&root, &term)?,
        Commands::BlastRadius { path } => cmd_blast_radius(&root, &path)?,
        Commands::Decisions => cmd_decisions(&root)?,
        Commands::Learn { note, file } => cmd_learn(&root, &note, file.as_deref())?,
        Commands::Warnings => cmd_warnings(&root)?,
        Commands::Watch => cmd_watch(&root)?,
    }

    Ok(())
}

// =================================================================
// Command implementations
// =================================================================

fn cmd_init(root: &PathBuf) -> Result<()> {
    println!("\n  {} {}\n", "ctx".cyan().bold(), "— Universal Agent Context Protocol");

    if Database::exists(root) {
        println!("  {} Already initialized. Running re-scan...\n", "⚡".yellow());
        return cmd_scan(root);
    }

    let start = Instant::now();
    let db = Database::open(root)?;

    println!("  {} Created {}", "✓".green(), ".ctx/ctx.db".dimmed());

    // Run initial analysis
    print!("  ⟳ Scanning project...");
    let result = analyzer::analyze_project(&db, root)?;
    println!(" {}", "done".green());

    println!("    {} files discovered", result.total_files.to_string().cyan());
    println!("    {} symbols extracted", result.total_symbols.to_string().cyan());
    println!("    {} dependencies mapped", result.total_imports.to_string().cyan());

    // Run git analysis
    print!("  ⟳ Analyzing git history...");
    let git_result = git::analyze_git_history(&db, root)?;
    println!(" {}", "done".green());

    if let Some(err) = &git_result.error {
        println!("    {} {}", "⚠".yellow(), err.dimmed());
    } else {
        println!("    {} commits analyzed", git_result.commits_analyzed.to_string().cyan());
        println!("    {} decisions extracted", git_result.decisions_found.to_string().cyan());
    }

    let elapsed = start.elapsed();
    println!("\n  {} Initialized in {:.1}s\n", "✓".green().bold(), elapsed.as_secs_f64());

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
            println!("  {} Added .ctx/ to .gitignore", "✓".green());
        }
    }

    Ok(())
}

fn cmd_scan(root: &PathBuf) -> Result<()> {
    let db = ensure_initialized(root)?;
    let start = Instant::now();

    print!("  ⟳ Scanning...");
    let result = analyzer::analyze_project(&db, root)?;
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

    // Also refresh git stats
    let git_result = git::analyze_git_history(&db, root)?;
    if git_result.error.is_none() {
        println!("    {} git commits analyzed", git_result.commits_analyzed.to_string().cyan());
    }

    let elapsed = start.elapsed();
    println!("  {} Completed in {:.1}s\n", "✓".green(), elapsed.as_secs_f64());

    Ok(())
}

fn cmd_map(root: &PathBuf) -> Result<()> {
    let db = ensure_initialized(root)?;
    let files = db.get_all_files()?;

    if files.is_empty() {
        println!("  {} No files found. Run {} first.", "!".yellow(), "ctx scan".cyan());
        return Ok(());
    }

    let project_name = root.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "project".to_string());

    println!("\n  {} {}\n", "📦".to_string(), project_name.cyan().bold());

    // Group files by directory
    let mut dir_map: std::collections::BTreeMap<String, Vec<&ctx::db::models::TrackedFile>> = std::collections::BTreeMap::new();
    for file in &files {
        let dir = std::path::Path::new(&file.path)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| ".".to_string());
        dir_map.entry(dir).or_default().push(file);
    }

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

        // Show symbols for each file
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
                    .filter(|s| s.parent_symbol_id.is_none()) // only top-level
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

    // Summary
    let lang_stats = db.language_stats()?;
    println!("\n  {}", "Languages:".white().bold());
    for (lang, count, lines) in &lang_stats {
        let bar_len = (*lines as f64 / lang_stats[0].2 as f64 * 20.0) as usize;
        let bar = "█".repeat(bar_len);
        println!("  {:>12}  {} {} files, {} lines", lang.cyan(), bar.green(), count, lines);
    }

    println!();
    Ok(())
}

fn cmd_status(root: &PathBuf) -> Result<()> {
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

    println!("\n  {} {} {}\n",
        "ctx".cyan().bold(),
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

    // Health overview
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
    Ok(())
}

fn cmd_query(root: &PathBuf, term: &str) -> Result<()> {
    let db = ensure_initialized(root)?;
    println!();
    query::execute_search(&db, term)?;
    println!();
    Ok(())
}

fn cmd_blast_radius(root: &PathBuf, path: &str) -> Result<()> {
    let db = ensure_initialized(root)?;
    query::execute_blast_radius(&db, path)?;
    println!();
    Ok(())
}

fn cmd_decisions(root: &PathBuf) -> Result<()> {
    let db = ensure_initialized(root)?;
    let decisions = db.get_decisions()?;

    if decisions.is_empty() {
        println!("\n  {} No decisions recorded yet.", "·".dimmed());
        println!("  Decisions are extracted from git commits (feat:, refactor:, BREAKING, etc.)");
        println!("  or added manually with: {}\n", "ctx learn \"decision description\"".cyan());
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
    Ok(())
}

fn cmd_learn(root: &PathBuf, note: &str, file: Option<&str>) -> Result<()> {
    let db = ensure_initialized(root)?;
    db.insert_knowledge(note, "manual", file)?;

    println!("\n  {} Knowledge recorded", "✓".green().bold());
    if let Some(f) = file {
        println!("  Related file: {}", f.cyan());
    }
    println!("  \"{}\"", note.white());
    println!();

    Ok(())
}

fn cmd_warnings(root: &PathBuf) -> Result<()> {
    let db = ensure_initialized(root)?;
    let health = db.get_file_health()?;
    let knowledge = db.get_warnings_knowledge()?;

    let fragile: Vec<_> = health.iter().filter(|h| h.is_fragile).collect();
    let dead: Vec<_> = health.iter().filter(|h| h.is_dead).collect();
    let large: Vec<_> = health.iter().filter(|h| h.line_count > 500).collect();

    let total_warnings = fragile.len() + dead.len() + large.len() + knowledge.len();

    if total_warnings == 0 {
        println!("\n  {} No warnings — looking good!\n", "✓".green().bold());
        return Ok(());
    }

    println!("\n  {} {} warnings\n", "⚠".yellow().bold(), total_warnings.to_string().yellow().bold());

    // Fragile files
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

    // Large files
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

    // Dead files
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

    // Agent knowledge warnings
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

    Ok(())
}

fn cmd_watch(root: &PathBuf) -> Result<()> {
    let db = ensure_initialized(root)?;
    drop(db); // Close db before watcher opens its own

    println!("\n  {} {}\n", "ctx".cyan().bold(), "— Watch Mode");
    watcher::watch_project(root)?;
    Ok(())
}
