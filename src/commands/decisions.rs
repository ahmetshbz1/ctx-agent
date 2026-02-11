use super::*;

pub(super) fn cmd_decisions(root: &Path, json_mode: bool) -> Result<()> {
    let db = ensure_initialized(root)?;
    let decisions = db.get_decisions()?;

    if json_mode {
        let entries: Vec<_> = decisions
            .iter()
            .map(|d| {
                json!({
                    "timestamp": d.timestamp,
                    "source": d.source,
                    "description": d.description,
                    "commit_hash": d.commit_hash,
                })
            })
            .collect();
        println!(
            "{}",
            json!({
                "command": "decisions",
                "count": entries.len(),
                "decisions": entries,
            })
        );
    } else {
        if decisions.is_empty() {
            println!("\n  {} No decisions recorded yet.", "·".dimmed());
            println!(
                "  Decisions are extracted from git commits (feat:, refactor:, BREAKING, etc.)"
            );
            println!(
                "  or added manually with: {}\n",
                "ctx-agent learn \"decision description\"".cyan()
            );
            return Ok(());
        }

        println!(
            "\n  Decisions: {}\n",
            decisions.len().to_string().cyan().bold()
        );

        for decision in decisions.iter().take(30) {
            let source_badge = match decision.source.as_str() {
                "commit" => "commit".blue(),
                "manual" => "manual".green(),
                _ => decision.source.as_str().into(),
            };
            let hash = decision
                .commit_hash
                .as_deref()
                .unwrap_or("")
                .chars()
                .take(8)
                .collect::<String>();
            let hash_str = if !hash.is_empty() {
                format!(" ({})", hash).dimmed().to_string()
            } else {
                String::new()
            };

            println!(
                "  {} [{}] {}{}",
                decision
                    .timestamp
                    .get(..10)
                    .unwrap_or(&decision.timestamp)
                    .dimmed(),
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
