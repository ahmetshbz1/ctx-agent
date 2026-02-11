mod rust_ext;
mod typescript;
mod python;

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

use anyhow::Result;
use tree_sitter::{Language, Parser, Node};

use crate::db::models::SymbolKind;

pub use rust_ext::extract_rust;
pub use typescript::extract_ts_js;
pub use python::extract_python;

/// A symbol extracted from parsing a file
#[derive(Debug, Clone)]
pub struct ExtractedSymbol {
    pub name: String,
    pub kind: SymbolKind,
    pub start_line: usize,
    pub end_line: usize,
    pub signature: String,
    pub children: Vec<ExtractedSymbol>,
}

/// An import/dependency extracted from a file
#[derive(Debug, Clone)]
pub struct ExtractedImport {
    pub path: String,
    pub kind: String,  // "import", "require", "use"
    pub names: Vec<String>,
}

/// Parse result for a single file
#[derive(Debug)]
pub struct ParseResult {
    pub symbols: Vec<ExtractedSymbol>,
    pub imports: Vec<ExtractedImport>,
}

/// Get tree-sitter language for a given language name
fn get_language(lang: &str) -> Option<Language> {
    match lang {
        "typescript" | "tsx" => Some(tree_sitter_typescript::language_typescript()),
        "javascript" | "jsx" => Some(tree_sitter_javascript::language()),
        "python" => Some(tree_sitter_python::language()),
        "rust" => Some(tree_sitter_rust::language()),
        _ => None,
    }
}

/// Parse a source file and extract symbols + imports
pub fn parse_file(source: &str, language: &str) -> Result<ParseResult> {
    let ts_lang = match get_language(language) {
        Some(l) => l,
        None => return Ok(ParseResult { symbols: vec![], imports: vec![] }),
    };

    let mut parser = Parser::new();
    parser.set_language(&ts_lang)?;

    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => return Ok(ParseResult { symbols: vec![], imports: vec![] }),
    };

    let root = tree.root_node();
    let source_bytes = source.as_bytes();

    let mut symbols = Vec::new();
    let mut imports = Vec::new();

    match language {
        "typescript" | "javascript" | "tsx" | "jsx" => extract_ts_js(root, source_bytes, &mut symbols, &mut imports),
        "python" => extract_python(root, source_bytes, &mut symbols, &mut imports),
        "rust" => extract_rust(root, source_bytes, &mut symbols, &mut imports),
        _ => {}
    }

    Ok(ParseResult { symbols, imports })
}

// ===========================================================================
// Shared utilities
// ===========================================================================

pub(crate) fn node_text(node: Node, source: &[u8]) -> String {
    node.utf8_text(source).unwrap_or("").to_string()
}
