pub mod models;
pub mod schema;

use anyhow::{Context, Result};
use rusqlite::Connection;
use std::path::{Path, PathBuf};

use self::models::*;

/// Main database handle
pub struct Database {
    conn: Connection,
    pub ctx_dir: PathBuf,
}

impl Database {
    /// Open or create the ctx database in the given project root
    pub fn open(project_root: &Path) -> Result<Self> {
        let ctx_dir = project_root.join(".ctx");
        std::fs::create_dir_all(&ctx_dir)
            .context("Failed to create .ctx directory")?;

        let db_path = ctx_dir.join("ctx.db");
        let conn = Connection::open(&db_path)
            .context("Failed to open database")?;

        // Enable WAL mode for better concurrent access
        conn.execute_batch("
            PRAGMA journal_mode=WAL;
            PRAGMA synchronous=NORMAL;
            PRAGMA foreign_keys=ON;
        ")?;

        schema::run_migrations(&conn)?;

        Ok(Self { conn, ctx_dir })
    }

    /// Check if the database exists for the project
    pub fn exists(project_root: &Path) -> bool {
        project_root.join(".ctx").join("ctx.db").exists()
    }

    // =================================================================
    // File operations
    // =================================================================

    /// Insert or update a file record
    pub fn upsert_file(&self, path: &str, language: &str, size_bytes: i64, hash: &str, line_count: i64) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO files (path, language, size_bytes, hash, line_count, last_analyzed)
             VALUES (?1, ?2, ?3, ?4, ?5, CURRENT_TIMESTAMP)
             ON CONFLICT(path) DO UPDATE SET
                language = ?2, size_bytes = ?3, hash = ?4, line_count = ?5,
                last_analyzed = CURRENT_TIMESTAMP",
            rusqlite::params![path, language, size_bytes, hash, line_count],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Get file by path
    pub fn get_file_by_path(&self, path: &str) -> Result<Option<TrackedFile>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, path, language, size_bytes, hash, line_count, last_analyzed FROM files WHERE path = ?1"
        )?;
        let result = stmt.query_row(rusqlite::params![path], |row| {
            Ok(TrackedFile {
                id: row.get(0)?,
                path: row.get(1)?,
                language: row.get(2)?,
                size_bytes: row.get(3)?,
                hash: row.get(4)?,
                line_count: row.get(5)?,
                last_analyzed: row.get(6)?,
            })
        });
        match result {
            Ok(f) => Ok(Some(f)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Get file ID by path
    pub fn get_file_id(&self, path: &str) -> Result<Option<i64>> {
        let mut stmt = self.conn.prepare("SELECT id FROM files WHERE path = ?1")?;
        let result = stmt.query_row(rusqlite::params![path], |row| row.get(0));
        match result {
            Ok(id) => Ok(Some(id)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Get all files
    pub fn get_all_files(&self) -> Result<Vec<TrackedFile>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, path, language, size_bytes, hash, line_count, last_analyzed FROM files ORDER BY path"
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(TrackedFile {
                id: row.get(0)?,
                path: row.get(1)?,
                language: row.get(2)?,
                size_bytes: row.get(3)?,
                hash: row.get(4)?,
                line_count: row.get(5)?,
                last_analyzed: row.get(6)?,
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Remove files not in the given list (for detecting deleted files)
    pub fn remove_files_not_in(&self, paths: &[String]) -> Result<usize> {
        if paths.is_empty() {
            return Ok(0);
        }
        let placeholders: Vec<String> = paths.iter().enumerate().map(|(i, _)| format!("?{}", i + 1)).collect();
        let sql = format!("DELETE FROM files WHERE path NOT IN ({})", placeholders.join(","));
        let mut stmt = self.conn.prepare(&sql)?;
        let params: Vec<&dyn rusqlite::types::ToSql> = paths.iter().map(|p| p as &dyn rusqlite::types::ToSql).collect();
        let count = stmt.execute(params.as_slice())?;
        Ok(count)
    }

    // =================================================================
    // Symbol operations
    // =================================================================

    /// Clear all symbols for a file
    pub fn clear_symbols(&self, file_id: i64) -> Result<()> {
        self.conn.execute("DELETE FROM symbols WHERE file_id = ?1", [file_id])?;
        Ok(())
    }

    /// Insert a symbol
    pub fn insert_symbol(&self, file_id: i64, name: &str, kind: &SymbolKind, start_line: i64, end_line: i64, signature: &str, parent_id: Option<i64>) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO symbols (file_id, name, kind, start_line, end_line, signature, parent_symbol_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![file_id, name, kind.as_str(), start_line, end_line, signature, parent_id],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Get all symbols for a file
    pub fn get_symbols_for_file(&self, file_id: i64) -> Result<Vec<Symbol>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, file_id, name, kind, start_line, end_line, signature, parent_symbol_id
             FROM symbols WHERE file_id = ?1 ORDER BY start_line"
        )?;
        let rows = stmt.query_map([file_id], |row| {
            let kind_str: String = row.get(3)?;
            Ok(Symbol {
                id: row.get(0)?,
                file_id: row.get(1)?,
                name: row.get(2)?,
                kind: SymbolKind::from_str(&kind_str),
                start_line: row.get(4)?,
                end_line: row.get(5)?,
                signature: row.get(6)?,
                parent_symbol_id: row.get(7)?,
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Count total symbols
    pub fn count_symbols(&self) -> Result<i64> {
        Ok(self.conn.query_row("SELECT COUNT(*) FROM symbols", [], |row| row.get(0))?)
    }

    /// Count symbols by kind
    pub fn count_symbols_by_kind(&self) -> Result<Vec<(String, i64)>> {
        let mut stmt = self.conn.prepare("SELECT kind, COUNT(*) FROM symbols GROUP BY kind ORDER BY COUNT(*) DESC")?;
        let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    // =================================================================
    // Dependency operations
    // =================================================================

    /// Clear dependencies for a file
    pub fn clear_dependencies(&self, file_id: i64) -> Result<()> {
        self.conn.execute("DELETE FROM dependencies WHERE from_file_id = ?1", [file_id])?;
        Ok(())
    }

    /// Insert a dependency
    pub fn insert_dependency(&self, from_file_id: i64, to_path: &str, kind: &str, imported_names: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO dependencies (from_file_id, to_path, kind, imported_names)
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![from_file_id, to_path, kind, imported_names],
        )?;
        Ok(())
    }

    /// Resolve dependency to_file_id based on path matching
    pub fn resolve_dependencies(&self) -> Result<()> {
        self.conn.execute(
            "UPDATE dependencies SET to_file_id = (
                SELECT f.id FROM files f WHERE f.path LIKE '%' || dependencies.to_path || '%'
                LIMIT 1
             ) WHERE to_file_id IS NULL",
            [],
        )?;
        Ok(())
    }

    /// Get files that depend on the given file
    pub fn get_dependents(&self, file_id: i64) -> Result<Vec<(i64, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT f.id, f.path FROM dependencies d
             JOIN files f ON f.id = d.from_file_id
             WHERE d.to_file_id = ?1"
        )?;
        let rows = stmt.query_map([file_id], |row| Ok((row.get(0)?, row.get(1)?)))?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Get files that this file depends on
    pub fn get_dependencies_of(&self, file_id: i64) -> Result<Vec<(Option<i64>, String)>> {
        let mut stmt = self.conn.prepare(
            "SELECT d.to_file_id, d.to_path FROM dependencies d WHERE d.from_file_id = ?1"
        )?;
        let rows = stmt.query_map([file_id], |row| Ok((row.get(0)?, row.get(1)?)))?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Count total dependencies
    pub fn count_dependencies(&self) -> Result<i64> {
        Ok(self.conn.query_row("SELECT COUNT(*) FROM dependencies", [], |row| row.get(0))?)
    }

    // =================================================================
    // Search operations (FTS5)
    // =================================================================

    /// Rebuild the FTS5 search index
    pub fn rebuild_search_index(&self) -> Result<()> {
        self.conn.execute("DELETE FROM search_index", [])?;
        self.conn.execute(
            "INSERT INTO search_index(name, path, kind, signature)
             SELECT s.name, f.path, s.kind, s.signature
             FROM symbols s JOIN files f ON f.id = s.file_id",
            [],
        )?;
        Ok(())
    }

    /// Full-text search across symbols
    pub fn search(&self, query: &str) -> Result<Vec<(String, String, String, String)>> {
        let fts_query = query.split_whitespace()
            .map(|w| format!("{}*", w))
            .collect::<Vec<_>>()
            .join(" ");

        let mut stmt = self.conn.prepare(
            "SELECT name, path, kind, signature FROM search_index WHERE search_index MATCH ?1 LIMIT 50"
        )?;
        let rows = stmt.query_map([&fts_query], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    // =================================================================
    // Decision operations
    // =================================================================

    /// Insert a decision
    pub fn insert_decision(&self, description: &str, source: &str, commit_hash: Option<&str>, related_files: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO decisions (description, source, commit_hash, related_files)
             VALUES (?1, ?2, ?3, ?4)",
            rusqlite::params![description, source, commit_hash, related_files],
        )?;
        Ok(())
    }

    /// Get all decisions
    pub fn get_decisions(&self) -> Result<Vec<Decision>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, timestamp, description, source, commit_hash, related_files
             FROM decisions ORDER BY timestamp DESC"
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(Decision {
                id: row.get(0)?,
                timestamp: row.get(1)?,
                description: row.get(2)?,
                source: row.get(3)?,
                commit_hash: row.get(4)?,
                related_files: row.get(5)?,
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    // =================================================================
    // Knowledge operations
    // =================================================================

    /// Insert a knowledge note
    pub fn insert_knowledge(&self, content: &str, source: &str, related_file: Option<&str>) -> Result<()> {
        self.conn.execute(
            "INSERT INTO knowledge (content, source, related_file) VALUES (?1, ?2, ?3)",
            rusqlite::params![content, source, related_file],
        )?;
        Ok(())
    }

    /// Get all knowledge notes
    pub fn get_knowledge(&self) -> Result<Vec<Knowledge>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, content, source, related_file, timestamp FROM knowledge ORDER BY timestamp DESC"
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(Knowledge {
                id: row.get(0)?,
                content: row.get(1)?,
                source: row.get(2)?,
                related_file: row.get(3)?,
                timestamp: row.get(4)?,
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Get warnings about knowledge
    pub fn get_warnings_knowledge(&self) -> Result<Vec<Knowledge>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, content, source, related_file, timestamp FROM knowledge
             WHERE source = 'agent' ORDER BY timestamp DESC"
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(Knowledge {
                id: row.get(0)?,
                content: row.get(1)?,
                source: row.get(2)?,
                related_file: row.get(3)?,
                timestamp: row.get(4)?,
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    // =================================================================
    // File stats operations
    // =================================================================

    /// Upsert file stats
    pub fn upsert_file_stats(&self, file_id: i64, commit_count: i64, last_modified: Option<&str>, churn_score: f64, contributors: i64) -> Result<()> {
        self.conn.execute(
            "INSERT INTO file_stats (file_id, commit_count, last_modified, churn_score, contributors)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(file_id) DO UPDATE SET
                commit_count = ?2, last_modified = ?3, churn_score = ?4, contributors = ?5",
            rusqlite::params![file_id, commit_count, last_modified, churn_score, contributors],
        )?;
        Ok(())
    }

    /// Get file health overview (for warnings)
    pub fn get_file_health(&self) -> Result<Vec<FileHealth>> {
        let mut stmt = self.conn.prepare(
            "SELECT f.path, f.language, f.line_count,
                    COALESCE(fs.commit_count, 0),
                    COALESCE(fs.churn_score, 0.0),
                    (SELECT COUNT(*) FROM dependencies d WHERE d.to_file_id = f.id)
             FROM files f
             LEFT JOIN file_stats fs ON fs.file_id = f.id
             ORDER BY fs.churn_score DESC NULLS LAST"
        )?;
        let rows = stmt.query_map([], |row| {
            let churn_score: f64 = row.get(4)?;
            let commit_count: i64 = row.get(3)?;
            let dependents_count: i64 = row.get(5)?;
            Ok(FileHealth {
                path: row.get(0)?,
                language: row.get(1)?,
                line_count: row.get(2)?,
                commit_count,
                churn_score,
                dependents_count,
                is_fragile: churn_score > 0.7 && dependents_count > 3,
                is_dead: commit_count == 0 && dependents_count == 0,
            })
        })?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    // =================================================================
    // Aggregate stats
    // =================================================================

    /// Count total files
    pub fn count_files(&self) -> Result<i64> {
        Ok(self.conn.query_row("SELECT COUNT(*) FROM files", [], |row| row.get(0))?)
    }

    /// Total lines of code
    pub fn total_lines(&self) -> Result<i64> {
        Ok(self.conn.query_row("SELECT COALESCE(SUM(line_count), 0) FROM files", [], |row| row.get(0))?)
    }

    /// Language breakdown
    pub fn language_stats(&self) -> Result<Vec<(String, i64, i64)>> {
        let mut stmt = self.conn.prepare(
            "SELECT language, COUNT(*), SUM(line_count) FROM files GROUP BY language ORDER BY SUM(line_count) DESC"
        )?;
        let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }
}
