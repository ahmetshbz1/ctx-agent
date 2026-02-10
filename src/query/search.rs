use anyhow::Result;
use colored::*;
use crate::db::Database;

/// Execute a search query and display results
pub fn execute_search(db: &Database, query: &str) -> Result<()> {
    let results = db.search(query)?;

    if results.is_empty() {
        println!("{}", "  No results found.".dimmed());
        return Ok(());
    }

    println!("  {} results for \"{}\":\n", results.len().to_string().cyan(), query.yellow());

    for (name, path, kind, signature) in &results {
        let icon = match kind.as_str() {
            "function" => "ƒ".cyan(),
            "method" => "ƒ".blue(),
            "class" => "C".magenta(),
            "struct" => "S".green(),
            "interface" => "I".yellow(),
            "enum" => "E".red(),
            "constant" => "K".white(),
            "type_alias" => "T".cyan(),
            "module" => "M".blue(),
            _ => "?".dimmed(),
        };
        println!("  {} {} {}", icon, signature.white().bold(), path.dimmed());
    }

    Ok(())
}
