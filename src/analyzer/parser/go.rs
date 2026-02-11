use super::{node_text, ExtractedImport, ExtractedSymbol};
use crate::db::models::SymbolKind;
use tree_sitter::Node;

/// Extract Go symbols and imports from a tree-sitter AST
pub fn extract_go(
    node: Node,
    source: &[u8],
    symbols: &mut Vec<ExtractedSymbol>,
    imports: &mut Vec<ExtractedImport>,
) {
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        match child.kind() {
            // ── Functions ──────────────────────────────────────────────
            "function_declaration" => {
                if let Some(sym) = extract_go_function(child, source) {
                    symbols.push(sym);
                }
            }

            // ── Methods (func (receiver) name(...) ...) ───────────────
            "method_declaration" => {
                if let Some(sym) = extract_go_method(child, source) {
                    symbols.push(sym);
                }
            }

            // ── Type declarations (struct, interface, type alias) ──────
            "type_declaration" => {
                extract_go_type_decl(child, source, symbols);
            }

            // ── Constants ──────────────────────────────────────────────
            "const_declaration" => {
                extract_go_const(child, source, symbols);
            }

            // ── Variables (top-level var blocks) ────────────────────────
            "var_declaration" => {
                extract_go_var(child, source, symbols);
            }

            // ── Package declaration ────────────────────────────────────
            "package_clause" => {
                let mut cursor = child.walk();
                for subchild in child.children(&mut cursor) {
                    if subchild.kind() == "package_identifier" {
                        let name = node_text(subchild, source);
                        symbols.push(ExtractedSymbol {
                            name,
                            kind: SymbolKind::Module,
                            start_line: child.start_position().row + 1,
                            end_line: child.end_position().row + 1,
                            signature: node_text(child, source),
                            children: vec![],
                        });
                        break;
                    }
                }
            }

            // ── Imports ────────────────────────────────────────────────
            "import_declaration" => {
                extract_go_imports(child, source, imports);
            }

            _ => {}
        }
    }
}

// ===========================================================================
// Function extraction
// ===========================================================================

fn extract_go_function(node: Node, source: &[u8]) -> Option<ExtractedSymbol> {
    let name_node = node.child_by_field_name("name")?;
    let name = node_text(name_node, source);

    let signature = build_func_signature(node, source, None);

    Some(ExtractedSymbol {
        name,
        kind: SymbolKind::Function,
        start_line: node.start_position().row + 1,
        end_line: node.end_position().row + 1,
        signature,
        children: vec![],
    })
}

fn extract_go_method(node: Node, source: &[u8]) -> Option<ExtractedSymbol> {
    let name_node = node.child_by_field_name("name")?;
    let name = node_text(name_node, source);

    // Get receiver type for signature
    let receiver = node
        .child_by_field_name("receiver")
        .map(|r| node_text(r, source));

    let signature = build_func_signature(node, source, receiver.as_deref());

    Some(ExtractedSymbol {
        name,
        kind: SymbolKind::Method,
        start_line: node.start_position().row + 1,
        end_line: node.end_position().row + 1,
        signature,
        children: vec![],
    })
}

fn build_func_signature(node: Node, source: &[u8], receiver: Option<&str>) -> String {
    let name = node
        .child_by_field_name("name")
        .map(|n| node_text(n, source))
        .unwrap_or_default();

    let params = node
        .child_by_field_name("parameters")
        .map(|n| node_text(n, source))
        .unwrap_or("()".to_string());

    let result = node
        .child_by_field_name("result")
        .map(|n| format!(" {}", node_text(n, source)))
        .unwrap_or_default();

    match receiver {
        Some(recv) => format!("func {} {}{}{}", recv, name, params, result),
        None => format!("func {}{}{}", name, params, result),
    }
}

// ===========================================================================
// Type declarations (struct, interface, type alias)
// ===========================================================================

fn extract_go_type_decl(node: Node, source: &[u8], symbols: &mut Vec<ExtractedSymbol>) {
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        if child.kind() == "type_spec" {
            if let Some(sym) = extract_type_spec(child, source) {
                symbols.push(sym);
            }
        }
    }
}

fn extract_type_spec(node: Node, source: &[u8]) -> Option<ExtractedSymbol> {
    let name_node = node.child_by_field_name("name")?;
    let name = node_text(name_node, source);

    let type_node = node.child_by_field_name("type")?;
    let type_text = type_node.kind();

    let (kind, children) = match type_text {
        "struct_type" => {
            let fields = extract_struct_fields(type_node, source);
            (SymbolKind::Struct, fields)
        }
        "interface_type" => {
            let methods = extract_interface_methods(type_node, source);
            (SymbolKind::Interface, methods)
        }
        _ => (SymbolKind::TypeAlias, vec![]),
    };

    // For structs, also look for methods defined elsewhere
    // (tree-sitter can't link them here, but we add fields as children)

    let signature = match kind {
        SymbolKind::Struct => format!("type {} struct", name),
        SymbolKind::Interface => format!("type {} interface", name),
        SymbolKind::TypeAlias => {
            let alias_text = node_text(type_node, source);
            let short = if alias_text.len() > 60 {
                format!("{}...", &alias_text[..57])
            } else {
                alias_text
            };
            format!("type {} {}", name, short)
        }
        _ => format!("type {}", name),
    };

    Some(ExtractedSymbol {
        name,
        kind,
        start_line: node.start_position().row + 1,
        end_line: node.end_position().row + 1,
        signature,
        children,
    })
}

fn extract_struct_fields(node: Node, source: &[u8]) -> Vec<ExtractedSymbol> {
    let mut fields = Vec::new();
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        if child.kind() == "field_declaration_list" {
            let mut inner_cursor = child.walk();
            for field in child.children(&mut inner_cursor) {
                if field.kind() == "field_declaration" {
                    // Get field name(s)
                    if let Some(name_node) = field.child_by_field_name("name") {
                        let name = node_text(name_node, source);
                        let type_str = field
                            .child_by_field_name("type")
                            .map(|t| node_text(t, source))
                            .unwrap_or_default();

                        fields.push(ExtractedSymbol {
                            name: name.clone(),
                            kind: SymbolKind::Constant, // Using constant for fields
                            start_line: field.start_position().row + 1,
                            end_line: field.end_position().row + 1,
                            signature: format!("{} {}", name, type_str),
                            children: vec![],
                        });
                    }
                }
            }
        }
    }

    fields
}

fn extract_interface_methods(node: Node, source: &[u8]) -> Vec<ExtractedSymbol> {
    let mut methods = Vec::new();
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        // Interface methods are method_spec nodes
        if child.kind() == "method_spec" || child.kind() == "method_elem" {
            if let Some(name_node) = child.child_by_field_name("name") {
                let name = node_text(name_node, source);
                let sig = node_text(child, source);

                methods.push(ExtractedSymbol {
                    name,
                    kind: SymbolKind::Method,
                    start_line: child.start_position().row + 1,
                    end_line: child.end_position().row + 1,
                    signature: sig,
                    children: vec![],
                });
            }
        }
    }

    methods
}

// ===========================================================================
// Constants and variables
// ===========================================================================

fn extract_go_const(node: Node, source: &[u8], symbols: &mut Vec<ExtractedSymbol>) {
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        if child.kind() == "const_spec" {
            if let Some(name_node) = child.child_by_field_name("name") {
                let name = node_text(name_node, source);
                let type_str = child
                    .child_by_field_name("type")
                    .map(|t| format!(" {}", node_text(t, source)))
                    .unwrap_or_default();

                let value = child
                    .child_by_field_name("value")
                    .map(|v| {
                        let val = node_text(v, source);
                        if val.len() > 40 {
                            format!(" = {}...", &val[..37])
                        } else {
                            format!(" = {}", val)
                        }
                    })
                    .unwrap_or_default();

                symbols.push(ExtractedSymbol {
                    name: name.clone(),
                    kind: SymbolKind::Constant,
                    start_line: child.start_position().row + 1,
                    end_line: child.end_position().row + 1,
                    signature: format!("const {}{}{}", name, type_str, value),
                    children: vec![],
                });
            }
        }
    }
}

fn extract_go_var(node: Node, source: &[u8], symbols: &mut Vec<ExtractedSymbol>) {
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        if child.kind() == "var_spec" {
            if let Some(name_node) = child.child_by_field_name("name") {
                let name = node_text(name_node, source);
                let type_str = child
                    .child_by_field_name("type")
                    .map(|t| node_text(t, source))
                    .unwrap_or_default();

                symbols.push(ExtractedSymbol {
                    name: name.clone(),
                    kind: SymbolKind::Constant,
                    start_line: child.start_position().row + 1,
                    end_line: child.end_position().row + 1,
                    signature: format!("var {} {}", name, type_str).trim().to_string(),
                    children: vec![],
                });
            }
        }
    }
}

// ===========================================================================
// Imports
// ===========================================================================

fn extract_go_imports(node: Node, source: &[u8], imports: &mut Vec<ExtractedImport>) {
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        match child.kind() {
            "import_spec" => {
                if let Some(imp) = extract_import_spec(child, source) {
                    imports.push(imp);
                }
            }
            "import_spec_list" => {
                let mut inner_cursor = child.walk();
                for spec in child.children(&mut inner_cursor) {
                    if spec.kind() == "import_spec" {
                        if let Some(imp) = extract_import_spec(spec, source) {
                            imports.push(imp);
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

fn extract_import_spec(node: Node, source: &[u8]) -> Option<ExtractedImport> {
    let path_node = node.child_by_field_name("path")?;
    let raw_path = node_text(path_node, source);
    // Strip quotes from import path
    let path = raw_path.trim_matches('"').to_string();

    // Check for alias
    let alias = node
        .child_by_field_name("name")
        .map(|n| node_text(n, source));

    let names = match alias {
        Some(a) if a == "." => vec!["*".to_string()], // dot import
        Some(a) if a == "_" => vec!["_".to_string()], // blank import
        Some(a) => vec![a],
        None => {
            // Use the last segment of the path as the default name
            let default_name = path.rsplit('/').next().unwrap_or(&path).to_string();
            vec![default_name]
        }
    };

    Some(ExtractedImport {
        path,
        kind: "import".to_string(),
        names,
    })
}
