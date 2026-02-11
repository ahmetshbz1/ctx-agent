use super::{node_text, ExtractedImport, ExtractedSymbol};
use crate::db::models::SymbolKind;
use tree_sitter::Node;

/// Extract C and C++ symbols and imports
pub fn extract_c_cpp(
    node: Node,
    source: &[u8],
    symbols: &mut Vec<ExtractedSymbol>,
    imports: &mut Vec<ExtractedImport>,
) {
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        match child.kind() {
            // ── Functions ──────────────────────────────────────────────
            "function_definition" => {
                if let Some(sym) = extract_function(child, source) {
                    symbols.push(sym);
                }
            }

            // ── Classes / Structs ──────────────────────────────────────
            "class_specifier" | "struct_specifier" => {
                if let Some(sym) = extract_class_struct(child, source) {
                    symbols.push(sym);
                }
            }

            // ── Typedefs / Type alias ──────────────────────────────────
            "type_definition" | "alias_declaration" => {
                if let Some(sym) = extract_typedef(child, source) {
                    symbols.push(sym);
                }
            }

            // ── Preprocessor Includes ──────────────────────────────────
            "preproc_include" => {
                extract_include(child, source, imports);
            }

            // ── Namespaces (C++) ───────────────────────────────────────
            "namespace_definition" => {
                extract_namespace(child, source, symbols, imports);
            }

            _ => {}
        }
    }
}

fn extract_function(node: Node, source: &[u8]) -> Option<ExtractedSymbol> {
    let declarator = node.child_by_field_name("declarator")?;

    // In C++, declarator can be complex. We need to find the "function_declarator" inside it.
    let func_decl = find_function_declarator(declarator)?;
    let name_node = func_decl.child_by_field_name("declarator")?;

    // Handle qualified names (Class::Method)
    let name = node_text(name_node, source);
    let signature = node_text(func_decl, source); // Rough signature

    // Determine if it's a method or function (heuristic)
    let kind = if name.contains("::") {
        SymbolKind::Method
    } else {
        SymbolKind::Function
    };

    Some(ExtractedSymbol {
        name,
        kind,
        start_line: node.start_position().row + 1,
        end_line: node.end_position().row + 1,
        signature,
        children: vec![],
    })
}

fn find_function_declarator(node: Node) -> Option<Node> {
    if node.kind() == "function_declarator" {
        return Some(node);
    }
    // Traverse down
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if let Some(found) = find_function_declarator(child) {
            return Some(found);
        }
    }
    None
}

fn extract_class_struct(node: Node, source: &[u8]) -> Option<ExtractedSymbol> {
    let name_node = node.child_by_field_name("name")?;
    let name = node_text(name_node, source);

    let kind = if node.kind() == "class_specifier" {
        SymbolKind::Class
    } else {
        SymbolKind::Struct
    };

    let signature = format!(
        "{} {}",
        if kind == SymbolKind::Class {
            "class"
        } else {
            "struct"
        },
        name
    );

    let mut children = Vec::new();
    // ... (rest of logic) ...
    // Extract fields/methods from body
    if let Some(body) = node.child_by_field_name("body") {
        let mut cursor = body.walk();
        for child in body.children(&mut cursor) {
            if child.kind() == "field_declaration" {
                // Fields
                if let Some(field_name) = extract_field_name(child, source) {
                    children.push(ExtractedSymbol {
                        name: field_name,
                        kind: SymbolKind::Constant, // Field
                        start_line: child.start_position().row + 1,
                        end_line: child.end_position().row + 1,
                        signature: node_text(child, source),
                        children: vec![],
                    });
                }
            } else if child.kind() == "function_definition" || child.kind() == "declaration" {
                // Methods defined inline
                if let Some(method) = extract_function(child, source) {
                    // It's a method inside a class
                    let mut m = method;
                    m.kind = SymbolKind::Method;
                    children.push(m);
                }
            }
        }
    }

    Some(ExtractedSymbol {
        name,
        kind,
        start_line: node.start_position().row + 1,
        end_line: node.end_position().row + 1,
        signature,
        children,
    })
}

fn extract_field_name(node: Node, source: &[u8]) -> Option<String> {
    // int x; -> declarator is x
    // simple field extraction attempt
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        if child.kind() == "field_identifier" {
            return Some(node_text(child, source));
        }
        // Recurse for complex declarators
        if child.kind().contains("declarator") {
            return Some(node_text(child, source));
        }
    }
    None
}

fn extract_typedef(node: Node, source: &[u8]) -> Option<ExtractedSymbol> {
    // typedef struct foo { ... } Bar;
    // using Bar = Foo;
    let name_node = node
        .child_by_field_name("declarator")
        .or_else(|| node.child_by_field_name("name"))?; // for using declaration

    let name = node_text(name_node, source);

    Some(ExtractedSymbol {
        name,
        kind: SymbolKind::TypeAlias,
        start_line: node.start_position().row + 1,
        end_line: node.end_position().row + 1,
        signature: node_text(node, source),
        children: vec![],
    })
}

fn extract_include(node: Node, source: &[u8], imports: &mut Vec<ExtractedImport>) {
    let path_node = node.child_by_field_name("path");
    if let Some(path) = path_node {
        let raw = node_text(path, source);
        // <stdio.h> or "my_header.h"
        let clean = raw
            .trim_matches(|c| c == '<' || c == '>' || c == '"')
            .to_string();

        imports.push(ExtractedImport {
            path: clean,
            kind: "include".to_string(),
            names: vec![],
        });
    }
}

fn extract_namespace(
    node: Node,
    source: &[u8],
    symbols: &mut Vec<ExtractedSymbol>,
    imports: &mut Vec<ExtractedImport>,
) {
    // namespace N { ... }
    let name = node
        .child_by_field_name("name")
        .map(|n| node_text(n, source))
        .unwrap_or_else(|| "anonymous".to_string());

    let mut inner_symbols = Vec::new();

    if let Some(body) = node.child_by_field_name("body") {
        extract_c_cpp(body, source, &mut inner_symbols, imports);
    }

    symbols.push(ExtractedSymbol {
        name,
        kind: SymbolKind::Module,
        start_line: node.start_position().row + 1,
        end_line: node.end_position().row + 1,
        signature: "namespace".to_string(),
        children: inner_symbols,
    });
}
