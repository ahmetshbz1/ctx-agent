pub mod db;
pub mod analyzer;
pub mod git;
pub mod query;
pub mod watcher;

// Re-export core types
pub use db::Database;
