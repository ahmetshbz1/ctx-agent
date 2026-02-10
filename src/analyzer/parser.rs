use anyhow::Result;
use tree_sitter::{Language, Parser, Node};

use crate::db::models::SymbolKind;

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

    extract_from_node(root, source_bytes, language, &mut symbols, &mut imports);

    Ok(ParseResult { symbols, imports })
}

fn extract_from_node(
    node: Node,
    source: &[u8],
    language: &str,
    symbols: &mut Vec<ExtractedSymbol>,
    imports: &mut Vec<ExtractedImport>,
) {
    match language {
        "typescript" | "javascript" => extract_ts_js(node, source, symbols, imports),
        "python" => extract_python(node, source, symbols, imports),
        "rust" => extract_rust(node, source, symbols, imports),
        _ => {}
    }
}

// ===========================================================================
// TypeScript / JavaScript extractor
// ===========================================================================

fn extract_ts_js(
    node: Node,
    source: &[u8],
    symbols: &mut Vec<ExtractedSymbol>,
    imports: &mut Vec<ExtractedImport>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "function_declaration" => {
                if let Some(sym) = extract_ts_function(child, source) {
                    symbols.push(sym);
                }
            }
            "class_declaration" => {
                if let Some(sym) = extract_ts_class(child, source) {
                    symbols.push(sym);
                }
            }
            "export_statement" => {
                // Look inside export for declarations
                let mut inner = child.walk();
                for export_child in child.children(&mut inner) {
                    match export_child.kind() {
                        "function_declaration" => {
                            if let Some(mut sym) = extract_ts_function(export_child, source) {
                                sym.signature = format!("export {}", sym.signature);
                                symbols.push(sym);
                            }
                        }
                        "class_declaration" => {
                            if let Some(mut sym) = extract_ts_class(export_child, source) {
                                sym.signature = format!("export {}", sym.signature);
                                symbols.push(sym);
                            }
                        }
                        "interface_declaration" => {
                            if let Some(sym) = extract_ts_interface(export_child, source) {
                                symbols.push(sym);
                            }
                        }
                        "type_alias_declaration" => {
                            if let Some(sym) = extract_ts_type_alias(export_child, source) {
                                symbols.push(sym);
                            }
                        }
                        "lexical_declaration" => {
                            extract_ts_lexical(export_child, source, symbols, true);
                        }
                        _ => {}
                    }
                }
            }
            "interface_declaration" => {
                if let Some(sym) = extract_ts_interface(child, source) {
                    symbols.push(sym);
                }
            }
            "type_alias_declaration" => {
                if let Some(sym) = extract_ts_type_alias(child, source) {
                    symbols.push(sym);
                }
            }
            "enum_declaration" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    let name = node_text(name_node, source);
                    symbols.push(ExtractedSymbol {
                        name: name.clone(),
                        kind: SymbolKind::Enum,
                        start_line: child.start_position().row + 1,
                        end_line: child.end_position().row + 1,
                        signature: format!("enum {}", name),
                        children: vec![],
                    });
                }
            }
            "lexical_declaration" => {
                extract_ts_lexical(child, source, symbols, false);
            }
            "import_statement" => {
                if let Some(imp) = extract_ts_import(child, source) {
                    imports.push(imp);
                }
            }
            _ => {}
        }
    }
}

fn extract_ts_function(node: Node, source: &[u8]) -> Option<ExtractedSymbol> {
    let name_node = node.child_by_field_name("name")?;
    let name = node_text(name_node, source);
    let params = node.child_by_field_name("parameters")
        .map(|n| node_text(n, source))
        .unwrap_or_else(|| "()".to_string());

    Some(ExtractedSymbol {
        name: name.clone(),
        kind: SymbolKind::Function,
        start_line: node.start_position().row + 1,
        end_line: node.end_position().row + 1,
        signature: format!("function {}{}", name, params),
        children: vec![],
    })
}

fn extract_ts_class(node: Node, source: &[u8]) -> Option<ExtractedSymbol> {
    let name_node = node.child_by_field_name("name")?;
    let name = node_text(name_node, source);

    // Extract methods
    let mut methods = Vec::new();
    if let Some(body) = node.child_by_field_name("body") {
        let mut cursor = body.walk();
        for child in body.children(&mut cursor) {
            if child.kind() == "method_definition" {
                if let Some(method_name) = child.child_by_field_name("name") {
                    let mname = node_text(method_name, source);
                    let params = child.child_by_field_name("parameters")
                        .map(|n| node_text(n, source))
                        .unwrap_or_else(|| "()".to_string());
                    methods.push(ExtractedSymbol {
                        name: mname.clone(),
                        kind: SymbolKind::Method,
                        start_line: child.start_position().row + 1,
                        end_line: child.end_position().row + 1,
                        signature: format!("{}{}", mname, params),
                        children: vec![],
                    });
                }
            }
        }
    }

    Some(ExtractedSymbol {
        name: name.clone(),
        kind: SymbolKind::Class,
        start_line: node.start_position().row + 1,
        end_line: node.end_position().row + 1,
        signature: format!("class {}", name),
        children: methods,
    })
}

fn extract_ts_interface(node: Node, source: &[u8]) -> Option<ExtractedSymbol> {
    let name_node = node.child_by_field_name("name")?;
    let name = node_text(name_node, source);
    Some(ExtractedSymbol {
        name: name.clone(),
        kind: SymbolKind::Interface,
        start_line: node.start_position().row + 1,
        end_line: node.end_position().row + 1,
        signature: format!("interface {}", name),
        children: vec![],
    })
}

fn extract_ts_type_alias(node: Node, source: &[u8]) -> Option<ExtractedSymbol> {
    let name_node = node.child_by_field_name("name")?;
    let name = node_text(name_node, source);
    Some(ExtractedSymbol {
        name: name.clone(),
        kind: SymbolKind::TypeAlias,
        start_line: node.start_position().row + 1,
        end_line: node.end_position().row + 1,
        signature: format!("type {}", name),
        children: vec![],
    })
}

fn extract_ts_lexical(node: Node, source: &[u8], symbols: &mut Vec<ExtractedSymbol>, exported: bool) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "variable_declarator" {
            if let Some(name_node) = child.child_by_field_name("name") {
                let name = node_text(name_node, source);
                // Check if the value is an arrow function or function
                let is_function = child.child_by_field_name("value")
                    .map(|v| matches!(v.kind(), "arrow_function" | "function"))
                    .unwrap_or(false);

                let kind = if is_function { SymbolKind::Function } else { SymbolKind::Constant };
                let prefix = if exported { "export " } else { "" };
                let keyword = if is_function { "const" } else { "const" };

                symbols.push(ExtractedSymbol {
                    name: name.clone(),
                    kind,
                    start_line: node.start_position().row + 1,
                    end_line: node.end_position().row + 1,
                    signature: format!("{}{} {}", prefix, keyword, name),
                    children: vec![],
                });
            }
        }
    }
}

fn extract_ts_import(node: Node, source: &[u8]) -> Option<ExtractedImport> {
    let mut path = String::new();
    let mut names = Vec::new();

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "string" => {
                path = node_text(child, source).trim_matches(|c| c == '\'' || c == '"').to_string();
            }
            "import_clause" => {
                let mut inner = child.walk();
                for clause_child in child.children(&mut inner) {
                    match clause_child.kind() {
                        "identifier" => names.push(node_text(clause_child, source)),
                        "named_imports" => {
                            let mut imports_cursor = clause_child.walk();
                            for import_spec in clause_child.children(&mut imports_cursor) {
                                if import_spec.kind() == "import_specifier" {
                                    if let Some(name_node) = import_spec.child_by_field_name("name") {
                                        names.push(node_text(name_node, source));
                                    }
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    if !path.is_empty() {
        Some(ExtractedImport { path, kind: "import".to_string(), names })
    } else {
        None
    }
}

// ===========================================================================
// Python extractor
// ===========================================================================

fn extract_python(
    node: Node,
    source: &[u8],
    symbols: &mut Vec<ExtractedSymbol>,
    imports: &mut Vec<ExtractedImport>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "function_definition" => {
                if let Some(sym) = extract_python_function(child, source) {
                    symbols.push(sym);
                }
            }
            "class_definition" => {
                if let Some(sym) = extract_python_class(child, source) {
                    symbols.push(sym);
                }
            }
            "import_statement" | "import_from_statement" => {
                if let Some(imp) = extract_python_import(child, source) {
                    imports.push(imp);
                }
            }
            _ => {}
        }
    }
}

fn extract_python_function(node: Node, source: &[u8]) -> Option<ExtractedSymbol> {
    let name_node = node.child_by_field_name("name")?;
    let name = node_text(name_node, source);
    let params = node.child_by_field_name("parameters")
        .map(|n| node_text(n, source))
        .unwrap_or_else(|| "()".to_string());

    Some(ExtractedSymbol {
        name: name.clone(),
        kind: SymbolKind::Function,
        start_line: node.start_position().row + 1,
        end_line: node.end_position().row + 1,
        signature: format!("def {}{}", name, params),
        children: vec![],
    })
}

fn extract_python_class(node: Node, source: &[u8]) -> Option<ExtractedSymbol> {
    let name_node = node.child_by_field_name("name")?;
    let name = node_text(name_node, source);

    let mut methods = Vec::new();
    if let Some(body) = node.child_by_field_name("body") {
        let mut cursor = body.walk();
        for child in body.children(&mut cursor) {
            if child.kind() == "function_definition" {
                if let Some(method) = extract_python_function(child, source) {
                    methods.push(ExtractedSymbol {
                        kind: SymbolKind::Method,
                        ..method
                    });
                }
            }
        }
    }

    Some(ExtractedSymbol {
        name: name.clone(),
        kind: SymbolKind::Class,
        start_line: node.start_position().row + 1,
        end_line: node.end_position().row + 1,
        signature: format!("class {}", name),
        children: methods,
    })
}

fn extract_python_import(node: Node, source: &[u8]) -> Option<ExtractedImport> {
    let text = node_text(node, source);
    let mut names = Vec::new();

    if node.kind() == "import_from_statement" {
        let mut path = String::new();
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            match child.kind() {
                "dotted_name" if path.is_empty() => {
                    path = node_text(child, source);
                }
                "dotted_name" => {
                    names.push(node_text(child, source));
                }
                "aliased_import" => {
                    if let Some(name_node) = child.child_by_field_name("name") {
                        names.push(node_text(name_node, source));
                    }
                }
                _ => {}
            }
        }
        if !path.is_empty() {
            return Some(ExtractedImport { path, kind: "import".to_string(), names });
        }
    } else {
        // Regular import
        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            if child.kind() == "dotted_name" {
                let path = node_text(child, source);
                return Some(ExtractedImport { path, kind: "import".to_string(), names: vec![] });
            }
        }
    }

    // Fallback: extract from text
    if text.starts_with("import ") || text.starts_with("from ") {
        let parts: Vec<&str> = text.split_whitespace().collect();
        if parts.len() >= 2 {
            return Some(ExtractedImport {
                path: parts[1].to_string(),
                kind: "import".to_string(),
                names,
            });
        }
    }

    None
}

// ===========================================================================
// Rust extractor
// ===========================================================================

fn extract_rust(
    node: Node,
    source: &[u8],
    symbols: &mut Vec<ExtractedSymbol>,
    imports: &mut Vec<ExtractedImport>,
) {
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        match child.kind() {
            "function_item" => {
                if let Some(sym) = extract_rust_function(child, source) {
                    symbols.push(sym);
                }
            }
            "struct_item" => {
                if let Some(name) = child.child_by_field_name("name") {
                    let n = node_text(name, source);
                    symbols.push(ExtractedSymbol {
                        name: n.clone(),
                        kind: SymbolKind::Struct,
                        start_line: child.start_position().row + 1,
                        end_line: child.end_position().row + 1,
                        signature: format!("struct {}", n),
                        children: vec![],
                    });
                }
            }
            "enum_item" => {
                if let Some(name) = child.child_by_field_name("name") {
                    let n = node_text(name, source);
                    symbols.push(ExtractedSymbol {
                        name: n.clone(),
                        kind: SymbolKind::Enum,
                        start_line: child.start_position().row + 1,
                        end_line: child.end_position().row + 1,
                        signature: format!("enum {}", n),
                        children: vec![],
                    });
                }
            }
            "impl_item" => {
                extract_rust_impl(child, source, symbols);
            }
            "trait_item" => {
                if let Some(name) = child.child_by_field_name("name") {
                    let n = node_text(name, source);
                    symbols.push(ExtractedSymbol {
                        name: n.clone(),
                        kind: SymbolKind::Interface,
                        start_line: child.start_position().row + 1,
                        end_line: child.end_position().row + 1,
                        signature: format!("trait {}", n),
                        children: vec![],
                    });
                }
            }
            "type_item" => {
                if let Some(name) = child.child_by_field_name("name") {
                    let n = node_text(name, source);
                    symbols.push(ExtractedSymbol {
                        name: n.clone(),
                        kind: SymbolKind::TypeAlias,
                        start_line: child.start_position().row + 1,
                        end_line: child.end_position().row + 1,
                        signature: format!("type {}", n),
                        children: vec![],
                    });
                }
            }
            "const_item" | "static_item" => {
                if let Some(name) = child.child_by_field_name("name") {
                    let n = node_text(name, source);
                    symbols.push(ExtractedSymbol {
                        name: n.clone(),
                        kind: SymbolKind::Constant,
                        start_line: child.start_position().row + 1,
                        end_line: child.end_position().row + 1,
                        signature: format!("const {}", n),
                        children: vec![],
                    });
                }
            }
            "use_declaration" => {
                let text = node_text(child, source);
                let path = text.trim_start_matches("use ")
                    .trim_end_matches(';')
                    .to_string();
                imports.push(ExtractedImport {
                    path,
                    kind: "use".to_string(),
                    names: vec![],
                });
            }
            "mod_item" => {
                if let Some(name) = child.child_by_field_name("name") {
                    let n = node_text(name, source);
                    symbols.push(ExtractedSymbol {
                        name: n.clone(),
                        kind: SymbolKind::Module,
                        start_line: child.start_position().row + 1,
                        end_line: child.end_position().row + 1,
                        signature: format!("mod {}", n),
                        children: vec![],
                    });
                }
            }
            _ => {}
        }
    }
}

fn extract_rust_function(node: Node, source: &[u8]) -> Option<ExtractedSymbol> {
    let name_node = node.child_by_field_name("name")?;
    let name = node_text(name_node, source);
    let params = node.child_by_field_name("parameters")
        .map(|n| node_text(n, source))
        .unwrap_or_else(|| "()".to_string());
    let ret = node.child_by_field_name("return_type")
        .map(|n| format!(" -> {}", node_text(n, source)))
        .unwrap_or_default();

    Some(ExtractedSymbol {
        name: name.clone(),
        kind: SymbolKind::Function,
        start_line: node.start_position().row + 1,
        end_line: node.end_position().row + 1,
        signature: format!("fn {}{}{}", name, params, ret),
        children: vec![],
    })
}

fn extract_rust_impl(node: Node, source: &[u8], symbols: &mut Vec<ExtractedSymbol>) {
    // Get the type name being implemented
    let type_name = node.child_by_field_name("type")
        .map(|n| node_text(n, source))
        .unwrap_or_else(|| "Unknown".to_string());

    if let Some(body) = node.child_by_field_name("body") {
        let mut cursor = body.walk();
        for child in body.children(&mut cursor) {
            if child.kind() == "function_item" {
                if let Some(mut method) = extract_rust_function(child, source) {
                    method.kind = SymbolKind::Method;
                    method.signature = format!("impl {} :: {}", type_name, method.signature);
                    symbols.push(method);
                }
            }
        }
    }
}

// ===========================================================================
// Utilities
// ===========================================================================

fn node_text(node: Node, source: &[u8]) -> String {
    node.utf8_text(source).unwrap_or("").to_string()
}
