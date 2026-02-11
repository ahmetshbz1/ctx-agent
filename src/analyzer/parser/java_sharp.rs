use super::{node_text, ExtractedImport, ExtractedSymbol};
use crate::db::models::SymbolKind;
use tree_sitter::Node;

pub fn extract_java_csharp(
    node: Node,
    source: &[u8],
    symbols: &mut Vec<ExtractedSymbol>,
    imports: &mut Vec<ExtractedImport>,
    language: &str, // "java" or "c_sharp"
) {
    let mut cursor = node.walk();

    for child in node.children(&mut cursor) {
        match child.kind() {
            // ── Classes / Interfaces / Structs / Enums ─────────────────
            "class_declaration"
            | "interface_declaration"
            | "struct_declaration"
            | "enum_declaration"
            | "record_declaration" => {
                if let Some(sym) = extract_type_decl(child, source, language) {
                    symbols.push(sym);
                }
            }

            // ── Namespaces (C#) ────────────────────────────────────────
            "namespace_declaration" | "file_scoped_namespace_declaration" => {
                extract_namespace_cs(child, source, symbols, imports);
            }

            // ── Package (Java) ─────────────────────────────────────────
            "package_declaration" => {
                if let Some(name_node) = child.child_by_field_name("name") {
                    let name = node_text(name_node, source);
                    symbols.push(ExtractedSymbol {
                        name,
                        kind: SymbolKind::Module,
                        start_line: child.start_position().row + 1,
                        end_line: child.end_position().row + 1,
                        signature: node_text(child, source),
                        children: vec![],
                    });
                }
            }

            // ── Functions (Top-level in C# 9+ or Java scripts) ──────────
            "method_declaration" | "local_function_statement" => {
                if let Some(sym) = extract_method(child, source, language) {
                    symbols.push(sym);
                }
            }

            // ── Imports ────────────────────────────────────────────────
            "import_declaration" => {
                // Java
                extract_java_import(child, source, imports);
            }
            "using_directive" => {
                // C#
                extract_csharp_using(child, source, imports);
            }

            _ => {}
        }
    }
}

fn extract_type_decl(node: Node, source: &[u8], language: &str) -> Option<ExtractedSymbol> {
    let name_node = node.child_by_field_name("name")?; // Both Java and C# use "name"
    let name = node_text(name_node, source);

    let kind_str = node.kind();
    let kind = match kind_str {
        "class_declaration" | "record_declaration" => SymbolKind::Class,
        "interface_declaration" => SymbolKind::Interface,
        "struct_declaration" => SymbolKind::Struct,
        "enum_declaration" => SymbolKind::Enum,
        _ => SymbolKind::Class,
    };

    let signature = format!("{} {}", kind_str.replace("_declaration", ""), name);

    let mut children = Vec::new();

    if let Some(body) = node.child_by_field_name("body") {
        let mut cursor = body.walk();
        for child in body.children(&mut cursor) {
            match child.kind() {
                "method_declaration" | "constructor_declaration" => {
                    if let Some(method) = extract_method(child, source, language) {
                        children.push(method);
                    }
                }
                "field_declaration" => {
                    // Extract fields
                    if let Some(field) = extract_field(child, source) {
                        children.push(field);
                    }
                }
                "property_declaration" => {
                    // C#
                    if let Some(prop) = extract_property(child, source) {
                        children.push(prop);
                    }
                }
                // Nested types
                "class_declaration"
                | "interface_declaration"
                | "struct_declaration"
                | "enum_declaration" => {
                    if let Some(inner) = extract_type_decl(child, source, language) {
                        children.push(inner);
                    }
                }
                _ => {}
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

fn extract_method(node: Node, source: &[u8], _language: &str) -> Option<ExtractedSymbol> {
    let name_node = node.child_by_field_name("name")?;
    let name = node_text(name_node, source);

    let signature = format!("method {}", name);

    Some(ExtractedSymbol {
        name,
        kind: SymbolKind::Method,
        start_line: node.start_position().row + 1,
        end_line: node.end_position().row + 1,
        signature,
        children: vec![],
    })
}

fn extract_field(node: Node, source: &[u8]) -> Option<ExtractedSymbol> {
    let declarator = node.child_by_field_name("declarator").or_else(|| {
        // Java: field_declaration -> variable_declarator -> name
        let mut c = node.walk();
        for child in node.children(&mut c) {
            if child.kind() == "variable_declarator" {
                return child.child_by_field_name("name");
            }
        }
        None
    })?;

    let name = node_text(declarator, source);
    let signature = format!("field {}", name);

    Some(ExtractedSymbol {
        name,
        kind: SymbolKind::Constant,
        start_line: node.start_position().row + 1,
        end_line: node.end_position().row + 1,
        signature,
        children: vec![],
    })
}

fn extract_property(node: Node, source: &[u8]) -> Option<ExtractedSymbol> {
    let name_node = node.child_by_field_name("name")?;
    let name = node_text(name_node, source);
    let signature = format!("prop {}", name);

    Some(ExtractedSymbol {
        name,
        kind: SymbolKind::Method, // Property usually behaves like methods
        start_line: node.start_position().row + 1,
        end_line: node.end_position().row + 1,
        signature,
        children: vec![],
    })
}

fn extract_namespace_cs(
    node: Node,
    source: &[u8],
    symbols: &mut Vec<ExtractedSymbol>,
    imports: &mut Vec<ExtractedImport>,
) {
    let name_node = node.child_by_field_name("name");
    let name = name_node
        .map(|n| node_text(n, source))
        .unwrap_or_else(|| "namespace".to_string());

    let mut inner_symbols = Vec::new();

    if let Some(body) = node.child_by_field_name("body") {
        extract_java_csharp(body, source, &mut inner_symbols, imports, "c_sharp");
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

fn extract_java_import(node: Node, source: &[u8], imports: &mut Vec<ExtractedImport>) {
    // import java.util.List;
    // static import java.lang.Math.*;
    if let Some(name) = node.child_by_field_name("name") {
        // or "path"? tree-sitter-java usually has name
        imports.push(ExtractedImport {
            path: node_text(name, source),
            kind: "import".to_string(),
            names: vec![],
        });
    }
}

fn extract_csharp_using(node: Node, source: &[u8], imports: &mut Vec<ExtractedImport>) {
    // using System;
    if let Some(name) = node.child_by_field_name("name") {
        // In C# it might be "name" or just child
        imports.push(ExtractedImport {
            path: node_text(name, source),
            kind: "using".to_string(),
            names: vec![],
        });
    }
}
