pub mod scanner;
pub mod parser;
pub mod graph;

use anyhow::{Context, Result};
use std::path::Path;

use crate::db::Database;
use crate::db::models::SymbolKind;
use scanner::ScannedFile;
use parser::{parse_file, ExtractedSymbol};

/// Run a full analysis of the project
pub fn analyze_project(db: &Database, root: &Path) -> Result<AnalysisResult> {
    let files = scanner::scan_project(root)?;

    let mut total_symbols = 0usize;
    let mut total_imports = 0usize;
    let mut analyzed_files = 0usize;
    let mut skipped_files = 0usize;
    let mut all_paths: Vec<String> = Vec::new();

    for file in &files {
        all_paths.push(file.relative_path.clone());

        // Upsert file into DB
        let file_id = db.upsert_file(
            &file.relative_path,
            &file.language,
            file.size_bytes as i64,
            &file.hash,
            file.line_count as i64,
        )?;

        // Check if file needs re-analysis (hash changed)
        let existing = db.get_file_by_path(&file.relative_path)?;
        let needs_reanalyze = existing.map(|f| f.hash != file.hash).unwrap_or(true);

        if !needs_reanalyze {
            skipped_files += 1;
            continue;
        }

        // Get the actual file_id (might be different from upsert return on conflict)
        let file_id = db.get_file_id(&file.relative_path)?
            .context("File should exist after upsert")?;

        // Clear old data for re-analysis
        db.clear_symbols(file_id)?;
        db.clear_dependencies(file_id)?;

        // Parse with tree-sitter if supported
        if scanner::is_parseable(&file.language) {
            match parse_file(&file.content, &file.language) {
                Ok(result) => {
                    // Store symbols
                    for sym in &result.symbols {
                        store_symbol(db, file_id, sym, None)?;
                        total_symbols += 1;
                        total_symbols += sym.children.len();
                    }

                    // Store imports as dependencies
                    for imp in &result.imports {
                        db.insert_dependency(
                            file_id,
                            &imp.path,
                            &imp.kind,
                            &serde_json::to_string(&imp.names).unwrap_or_else(|_| "[]".to_string()),
                        )?;
                        total_imports += 1;
                    }
                }
                Err(_) => {
                    // Parsing failed, but file is still tracked
                }
            }
        }

        analyzed_files += 1;
    }

    // Remove files that no longer exist
    let removed = db.remove_files_not_in(&all_paths)?;

    // Resolve dependency links
    db.resolve_dependencies()?;

    // Rebuild search index
    db.rebuild_search_index()?;

    Ok(AnalysisResult {
        total_files: files.len(),
        analyzed_files,
        skipped_files,
        removed_files: removed,
        total_symbols,
        total_imports,
    })
}

/// Recursively store a symbol and its children
fn store_symbol(db: &Database, file_id: i64, sym: &ExtractedSymbol, parent_id: Option<i64>) -> Result<()> {
    let sym_id = db.insert_symbol(
        file_id,
        &sym.name,
        &sym.kind,
        sym.start_line as i64,
        sym.end_line as i64,
        &sym.signature,
        parent_id,
    )?;

    for child in &sym.children {
        store_symbol(db, file_id, child, Some(sym_id))?;
    }

    Ok(())
}

/// Result of a project analysis
#[derive(Debug)]
pub struct AnalysisResult {
    pub total_files: usize,
    pub analyzed_files: usize,
    pub skipped_files: usize,
    pub removed_files: usize,
    pub total_symbols: usize,
    pub total_imports: usize,
}
