use anyhow::Result;
use rusqlite::Connection;

/// Run all schema migrations
pub fn run_migrations(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS files (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            path            TEXT NOT NULL UNIQUE,
            language        TEXT NOT NULL DEFAULT 'unknown',
            size_bytes      INTEGER NOT NULL DEFAULT 0,
            hash            TEXT NOT NULL DEFAULT '',
            line_count      INTEGER NOT NULL DEFAULT 0,
            last_analyzed   DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE IF NOT EXISTS symbols (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            file_id         INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
            name            TEXT NOT NULL,
            kind            TEXT NOT NULL,
            start_line      INTEGER NOT NULL,
            end_line        INTEGER NOT NULL,
            signature       TEXT NOT NULL DEFAULT '',
            parent_symbol_id INTEGER REFERENCES symbols(id) ON DELETE SET NULL
        );

        CREATE TABLE IF NOT EXISTS dependencies (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            from_file_id    INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
            to_path         TEXT NOT NULL,
            to_file_id      INTEGER REFERENCES files(id) ON DELETE SET NULL,
            kind            TEXT NOT NULL DEFAULT 'import',
            imported_names  TEXT NOT NULL DEFAULT '[]'
        );

        CREATE TABLE IF NOT EXISTS decisions (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp       DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
            description     TEXT NOT NULL,
            source          TEXT NOT NULL DEFAULT 'manual',
            commit_hash     TEXT,
            related_files   TEXT NOT NULL DEFAULT '[]'
        );

        CREATE TABLE IF NOT EXISTS meta (
            key             TEXT PRIMARY KEY,
            value           TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS knowledge (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            content         TEXT NOT NULL,
            source          TEXT NOT NULL DEFAULT 'manual',
            related_file    TEXT,
            timestamp       DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
        );

        CREATE TABLE IF NOT EXISTS file_stats (
            file_id         INTEGER PRIMARY KEY REFERENCES files(id) ON DELETE CASCADE,
            commit_count    INTEGER NOT NULL DEFAULT 0,
            last_modified   DATETIME,
            churn_score     REAL NOT NULL DEFAULT 0.0,
            contributors    INTEGER NOT NULL DEFAULT 0
        );

        -- Indexes for fast lookups
        CREATE INDEX IF NOT EXISTS idx_symbols_file_id ON symbols(file_id);
        CREATE INDEX IF NOT EXISTS idx_symbols_name ON symbols(name);
        CREATE INDEX IF NOT EXISTS idx_symbols_kind ON symbols(kind);
        CREATE INDEX IF NOT EXISTS idx_deps_from ON dependencies(from_file_id);
        CREATE INDEX IF NOT EXISTS idx_deps_to ON dependencies(to_file_id);
        CREATE INDEX IF NOT EXISTS idx_knowledge_file ON knowledge(related_file);
    ",
    )?;

    // Keep only one decision row per commit hash before enabling uniqueness.
    conn.execute_batch(
        "
        DELETE FROM decisions
        WHERE source = 'commit'
          AND commit_hash IS NOT NULL
          AND id NOT IN (
              SELECT MIN(id) FROM decisions
              WHERE source = 'commit' AND commit_hash IS NOT NULL
              GROUP BY commit_hash
          );

        CREATE UNIQUE INDEX IF NOT EXISTS idx_decisions_commit_hash_unique
            ON decisions(commit_hash)
            WHERE source = 'commit' AND commit_hash IS NOT NULL;
    ",
    )?;

    // FTS5 virtual table for full-text search
    conn.execute_batch(
        "
        CREATE VIRTUAL TABLE IF NOT EXISTS search_index USING fts5(
            name,
            path,
            kind,
            signature,
            tokenize='porter unicode61'
        );
    ",
    )?;

    Ok(())
}
