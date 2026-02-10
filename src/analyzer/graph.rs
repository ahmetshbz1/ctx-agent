use std::collections::{HashMap, HashSet, VecDeque};

use anyhow::Result;
use crate::db::Database;

/// Compute the blast radius of a file: all files that would be affected
/// if this file changes (transitive dependents)
pub fn blast_radius(db: &Database, file_id: i64) -> Result<Vec<(i64, String, usize)>> {
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();
    let mut result = Vec::new();

    visited.insert(file_id);
    queue.push_back((file_id, 0usize)); // (file_id, depth)

    while let Some((current_id, depth)) = queue.pop_front() {
        let dependents = db.get_dependents(current_id)?;
        for (dep_id, dep_path) in dependents {
            if visited.insert(dep_id) {
                result.push((dep_id, dep_path, depth + 1));
                queue.push_back((dep_id, depth + 1));
            }
        }
    }

    // Sort by depth (closest first)
    result.sort_by_key(|r| r.2);
    Ok(result)
}

/// Build a map visualization of the dependency tree
pub fn dependency_tree_display(db: &Database, file_id: i64) -> Result<Vec<String>> {
    let mut lines = Vec::new();
    let dependents = db.get_dependents(file_id)?;
    let deps = db.get_dependencies_of(file_id)?;

    if !deps.is_empty() {
        lines.push("  ← depends on:".to_string());
        for (_, path) in &deps {
            lines.push(format!("    ← {}", path));
        }
    }

    if !dependents.is_empty() {
        lines.push("  → depended on by:".to_string());
        for (_, path) in &dependents {
            lines.push(format!("    → {}", path));
        }
    }

    Ok(lines)
}
