use tree_sitter::Node;

use crate::db::models::SymbolKind;
use super::{ExtractedSymbol, ExtractedImport, node_text};

// ===========================================================================
// TypeScript / JavaScript extractor
// ===========================================================================

pub fn extract_ts_js(
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
                extract_ts_export(child, source, symbols, imports);
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

/// Handle export statements — including default exports and re-exports
fn extract_ts_export(
    node: Node,
    source: &[u8],
    symbols: &mut Vec<ExtractedSymbol>,
    imports: &mut Vec<ExtractedImport>,
) {
    let mut inner = node.walk();
    let mut has_declaration = false;

    for export_child in node.children(&mut inner) {
        match export_child.kind() {
            "function_declaration" => {
                if let Some(mut sym) = extract_ts_function(export_child, source) {
                    sym.signature = format!("export {}", sym.signature);
                    symbols.push(sym);
                    has_declaration = true;
                }
            }
            "class_declaration" => {
                if let Some(mut sym) = extract_ts_class(export_child, source) {
                    sym.signature = format!("export {}", sym.signature);
                    symbols.push(sym);
                    has_declaration = true;
                }
            }
            "interface_declaration" => {
                if let Some(sym) = extract_ts_interface(export_child, source) {
                    symbols.push(sym);
                    has_declaration = true;
                }
            }
            "type_alias_declaration" => {
                if let Some(sym) = extract_ts_type_alias(export_child, source) {
                    symbols.push(sym);
                    has_declaration = true;
                }
            }
            "lexical_declaration" => {
                extract_ts_lexical(export_child, source, symbols, true);
                has_declaration = true;
            }
            // Default export: export default function() {} or export default class {}
            "function" | "arrow_function" => {
                symbols.push(ExtractedSymbol {
                    name: "default".to_string(),
                    kind: SymbolKind::Function,
                    start_line: node.start_position().row + 1,
                    end_line: node.end_position().row + 1,
                    signature: "export default function".to_string(),
                    children: vec![],
                });
                has_declaration = true;
            }
            _ => {}
        }
    }

    // Re-export: export { foo } from './bar'  or  export * from './bar'
    if !has_declaration {
        let text = node_text(node, source);
        if text.contains(" from ") {
            // Extract the source path
            if let Some(source_node) = node.children(&mut node.walk())
                .find(|c| c.kind() == "string")
            {
                let path = node_text(source_node, source)
                    .trim_matches(|c| c == '\'' || c == '"')
                    .to_string();
                let mut names = Vec::new();

                // Extract re-exported names
                let mut cursor2 = node.walk();
                for child in node.children(&mut cursor2) {
                    if child.kind() == "export_clause" {
                        let mut ec = child.walk();
                        for spec in child.children(&mut ec) {
                            if spec.kind() == "export_specifier" {
                                if let Some(name_node) = spec.child_by_field_name("name") {
                                    names.push(node_text(name_node, source));
                                }
                            }
                        }
                    }
                }

                imports.push(ExtractedImport {
                    path,
                    kind: "re-export".to_string(),
                    names,
                });
            }
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
                let is_function = child.child_by_field_name("value")
                    .map(|v| matches!(v.kind(), "arrow_function" | "function"))
                    .unwrap_or(false);

                let kind = if is_function { SymbolKind::Function } else { SymbolKind::Constant };
                let prefix = if exported { "export " } else { "" };

                symbols.push(ExtractedSymbol {
                    name: name.clone(),
                    kind,
                    start_line: node.start_position().row + 1,
                    end_line: node.end_position().row + 1,
                    signature: format!("{}const {}", prefix, name),
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
                        // Namespace import: import * as foo from 'bar'
                        "namespace_import" => {
                            let mut ns_cursor = clause_child.walk();
                            for ns_child in clause_child.children(&mut ns_cursor) {
                                if ns_child.kind() == "identifier" {
                                    names.push(format!("* as {}", node_text(ns_child, source)));
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
