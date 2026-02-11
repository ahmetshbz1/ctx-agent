use tree_sitter::Node;

use super::{node_text, ExtractedImport, ExtractedSymbol};
use crate::db::models::SymbolKind;

// ===========================================================================
// Rust extractor
// ===========================================================================

pub fn extract_rust(
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
                let path = text
                    .trim_start_matches("use ")
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

                    // External module declarations (e.g. `mod foo;`) are real file dependencies.
                    if child.child_by_field_name("body").is_none() {
                        imports.push(ExtractedImport {
                            path: n,
                            kind: "mod".to_string(),
                            names: vec![],
                        });
                    }
                }
            }
            _ => {}
        }
    }
}

fn extract_rust_function(node: Node, source: &[u8]) -> Option<ExtractedSymbol> {
    let name_node = node.child_by_field_name("name")?;
    let name = node_text(name_node, source);
    let params = node
        .child_by_field_name("parameters")
        .map(|n| node_text(n, source))
        .unwrap_or_else(|| "()".to_string());
    let ret = node
        .child_by_field_name("return_type")
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
    let type_name = node
        .child_by_field_name("type")
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
