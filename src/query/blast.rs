use anyhow::Result;
use colored::*;
use crate::db::Database;
use crate::analyzer::graph;

/// Execute blast-radius analysis and display results
pub fn execute_blast_radius(db: &Database, file_path: &str) -> Result<()> {
    let file_id = match db.get_file_id(file_path)? {
        Some(id) => id,
        None => {
            println!("  {} File not found: {}", "✗".red(), file_path);
            return Ok(());
        }
    };

    // Direct dependencies
    let deps = db.get_dependencies_of(file_id)?;
    let dependents = db.get_dependents(file_id)?;

    println!("\n  {} {}\n", "Blast Radius:".yellow().bold(), file_path.white().bold());

    // Show what this file depends on
    if !deps.is_empty() {
        println!("  {} {} dependencies (this file imports):", "←".blue(), deps.len().to_string().cyan());
        for (_, path) in &deps {
            println!("    {} {}", "←".dimmed(), path);
        }
        println!();
    }

    // Show direct dependents
    if !dependents.is_empty() {
        println!("  {} {} direct dependents (files that import this):", "→".green(), dependents.len().to_string().cyan());
        for (_, path) in &dependents {
            println!("    {} {}", "→".dimmed(), path);
        }
        println!();
    }

    // Show transitive blast radius
    let radius = graph::blast_radius(db, file_id)?;
    if !radius.is_empty() {
        let max_depth = radius.iter().map(|r| r.2).max().unwrap_or(0);
        println!("  {} {} total files in blast radius (depth {}):",
            "💥".to_string().red(),
            radius.len().to_string().red().bold(),
            max_depth.to_string().yellow()
        );
        for (_, path, depth) in &radius {
            let indent = "  ".repeat(*depth);
            let marker = if *depth == 1 { "→" } else { "↳" };
            println!("    {}{} {}", indent, marker.dimmed(), path);
        }
        println!();

        // Risk assessment
        let risk = if radius.len() > 20 {
            "CRITICAL".red().bold()
        } else if radius.len() > 10 {
            "HIGH".red()
        } else if radius.len() > 5 {
            "MEDIUM".yellow()
        } else {
            "LOW".green()
        };
        println!("  Risk: {}", risk);
    } else if dependents.is_empty() {
        println!("  {} No files depend on this file (leaf node)", "✓".green());
    }

    Ok(())
}
