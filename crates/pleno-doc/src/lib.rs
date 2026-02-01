//! # Pleno Documentation Generator
//!
//! This crate provides functionality to extract documentation from Pleno source files
//! and generate Markdown documentation.

use pleno_ast::{
    EnumDecl, FieldDecl, FunctionDecl, GenericParam, Param, Program, PropertyDecl, Stmt, StmtKind,
    Visibility,
};
use std::fmt::Write;

/// Represents the kind of documented item
#[derive(Debug, Clone, PartialEq)]
pub enum DocItemKind {
    Module,
    Class,
    Interface,
    Enum,
    Function,
    Method,
    Property,
    Field,
    TypeAlias,
    Extension,
    Macro,
    Constant,
    Variable,
}

impl std::fmt::Display for DocItemKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DocItemKind::Module => write!(f, "Module"),
            DocItemKind::Class => write!(f, "Class"),
            DocItemKind::Interface => write!(f, "Interface"),
            DocItemKind::Enum => write!(f, "Enum"),
            DocItemKind::Function => write!(f, "Function"),
            DocItemKind::Method => write!(f, "Method"),
            DocItemKind::Property => write!(f, "Property"),
            DocItemKind::Field => write!(f, "Field"),
            DocItemKind::TypeAlias => write!(f, "Type Alias"),
            DocItemKind::Extension => write!(f, "Extension"),
            DocItemKind::Macro => write!(f, "Macro"),
            DocItemKind::Constant => write!(f, "Constant"),
            DocItemKind::Variable => write!(f, "Variable"),
        }
    }
}

/// A parameter documented in a doc comment
#[derive(Debug, Clone)]
pub struct DocParam {
    pub name: String,
    pub description: String,
}

/// Return value documentation extracted from a doc comment
#[derive(Debug, Clone)]
pub struct DocReturns {
    pub description: String,
}

/// A documented item extracted from the AST
#[derive(Debug, Clone)]
pub struct DocItem {
    /// Name of the item
    pub name: String,
    /// Kind of item (class, function, etc.)
    pub kind: DocItemKind,
    /// Human-readable signature
    pub signature: String,
    /// Documentation comment content (cleaned of tags)
    pub doc: Option<String>,
    /// Visibility (public, private, protected)
    pub visibility: Option<Visibility>,
    /// Generic parameters
    pub generics: Vec<String>,
    /// Parameters from tags
    pub params: Vec<DocParam>,
    /// Return info from tags
    pub returns: Option<DocReturns>,
    /// Child items (methods, fields, etc.)
    pub children: Vec<DocItem>,
    /// Additional metadata (e.g., "abstract", "static", "async")
    pub modifiers: Vec<String>,
}

impl DocItem {
    pub fn new(name: String, kind: DocItemKind) -> Self {
        Self {
            name,
            kind,
            signature: String::new(),
            doc: None,
            visibility: None,
            generics: Vec::new(),
            params: Vec::new(),
            returns: None,
            children: Vec::new(),
            modifiers: Vec::new(),
        }
    }
}

/// Generates documentation from a parsed AST
pub struct DocGenerator;

impl DocGenerator {
    pub fn new() -> Self {
        Self
    }

    /// Extract documentation items from a program
    pub fn extract(&self, program: &Program) -> Vec<DocItem> {
        let mut items = Vec::new();
        for stmt in &program.statements {
            if let Some(item) = self.extract_stmt(stmt) {
                items.push(item);
            }
        }
        items
    }

    fn process_doc(
        &self,
        doc: Option<String>,
    ) -> (Option<String>, Vec<DocParam>, Option<DocReturns>) {
        let Some(doc_str) = doc else {
            return (None, Vec::new(), None);
        };

        let mut main_doc = String::new();
        let mut params = Vec::new();
        let mut returns = None;

        for line in doc_str.lines() {
            let line = line.trim();
            if let Some(stripped) = line.strip_prefix("@param") {
                let content = stripped.trim();
                let parts: Vec<&str> = content.splitn(2, ' ').collect();
                if !parts.is_empty() && !parts[0].is_empty() {
                    params.push(DocParam {
                        name: parts[0].to_string(),
                        description: parts.get(1).map(|s| s.to_string()).unwrap_or_default(),
                    });
                }
            } else if let Some(stripped) = line
                .strip_prefix("@returns")
                .or_else(|| line.strip_prefix("@return"))
            {
                let desc = stripped.trim().to_string();
                if !desc.is_empty() {
                    returns = Some(DocReturns { description: desc });
                }
            } else {
                let cleaned_line = if let Some(stripped) = line.strip_prefix("///") {
                    stripped.trim()
                } else if let Some(stripped) = line.strip_prefix("//") {
                    stripped.trim()
                } else {
                    line
                };

                if !cleaned_line.is_empty() || !main_doc.is_empty() {
                    if !main_doc.is_empty() {
                        main_doc.push('\n');
                    }
                    main_doc.push_str(cleaned_line);
                }
            }
        }

        (
            if main_doc.trim().is_empty() {
                None
            } else {
                Some(main_doc.trim().to_string())
            },
            params,
            returns,
        )
    }

    fn extract_stmt(&self, stmt: &Stmt) -> Option<DocItem> {
        match &stmt.kind {
            StmtKind::Module { name, body, doc } => {
                let mut item = DocItem::new(name.clone(), DocItemKind::Module);
                let (main_doc, params, returns) = self.process_doc(doc.clone());
                item.doc = main_doc;
                item.params = params;
                item.returns = returns;
                item.signature = format!("module {}", name);
                if let Some(body) = body {
                    for stmt in body {
                        if let Some(child) = self.extract_stmt(stmt) {
                            item.children.push(child);
                        }
                    }
                }
                Some(item)
            }
            StmtKind::Class {
                name,
                generics,
                fields,
                methods,
                properties,
                superclass,
                implements,
                is_abstract,
                doc,
                ..
            } => {
                let mut item = DocItem::new(name.clone(), DocItemKind::Class);
                let (main_doc, params, returns) = self.process_doc(doc.clone());
                item.doc = main_doc;
                item.params = params;
                item.returns = returns;
                item.generics = generics.iter().map(format_generic).collect();
                item.signature = self.format_class_signature(
                    name,
                    generics,
                    superclass.as_deref(),
                    implements,
                    *is_abstract,
                );
                if *is_abstract {
                    item.modifiers.push("abstract".to_string());
                }
                // Extract fields
                for field in fields {
                    item.children.push(self.extract_field(field));
                }
                // Extract properties
                for prop in properties {
                    item.children.push(self.extract_property(prop));
                }
                // Extract methods
                for method in methods {
                    let mut method_item = self.extract_function(method);
                    method_item.kind = DocItemKind::Method;
                    item.children.push(method_item);
                }
                Some(item)
            }
            StmtKind::Interface {
                name,
                generics,
                methods,
                extends,
                doc,
                ..
            } => {
                let mut item = DocItem::new(name.clone(), DocItemKind::Interface);
                let (main_doc, params, returns) = self.process_doc(doc.clone());
                item.doc = main_doc;
                item.params = params;
                item.returns = returns;
                item.generics = generics.iter().map(format_generic).collect();
                item.signature = self.format_interface_signature(name, generics, extends);
                for method in methods {
                    let mut method_item = self.extract_function(method);
                    method_item.kind = DocItemKind::Method;
                    item.children.push(method_item);
                }
                Some(item)
            }
            StmtKind::Enum(enum_decl) => Some(self.extract_enum(enum_decl)),
            StmtKind::Fn {
                name,
                generics,
                params,
                is_async,
                doc,
                ..
            } => {
                let mut item = DocItem::new(name.clone(), DocItemKind::Function);
                let (main_doc, doc_params, doc_returns) = self.process_doc(doc.clone());
                item.doc = main_doc;
                item.params = doc_params;
                item.returns = doc_returns;
                item.generics = generics.iter().map(format_generic).collect();
                item.signature = self.format_function_signature(name, generics, params, None);
                if *is_async {
                    item.modifiers.push("async".to_string());
                }
                Some(item)
            }
            StmtKind::TypeAlias { name, ty, doc } => {
                let mut item = DocItem::new(name.clone(), DocItemKind::TypeAlias);
                let (main_doc, doc_params, doc_returns) = self.process_doc(doc.clone());
                item.doc = main_doc;
                item.params = doc_params;
                item.returns = doc_returns;
                item.signature = format!("type {} = {}", name, ty);
                Some(item)
            }
            StmtKind::Extend {
                target,
                generics,
                methods,
                doc,
            } => {
                let mut item = DocItem::new(target.clone(), DocItemKind::Extension);
                let (main_doc, doc_params, doc_returns) = self.process_doc(doc.clone());
                item.doc = main_doc;
                item.params = doc_params;
                item.returns = doc_returns;
                item.generics = generics.iter().map(format_generic).collect();
                item.signature = format!("extend {}", target);
                for method in methods {
                    let mut method_item = self.extract_function(method);
                    method_item.kind = DocItemKind::Method;
                    item.children.push(method_item);
                }
                Some(item)
            }
            StmtKind::Let {
                name,
                ty,
                is_const,
                doc,
                ..
            } => {
                if doc.is_some() {
                    let kind = if *is_const {
                        DocItemKind::Constant
                    } else {
                        DocItemKind::Variable
                    };
                    let mut item = DocItem::new(name.clone(), kind);
                    item.doc = doc.clone();
                    item.signature = if let Some(ty) = ty {
                        format!("{}: {}", name, ty)
                    } else {
                        name.clone()
                    };
                    if *is_const {
                        item.modifiers.push("const".to_string());
                    }
                    Some(item)
                } else {
                    None
                }
            }
            StmtKind::MacroDecl {
                name,
                params: ast_params,
                doc,
                ..
            } => {
                let mut item = DocItem::new(name.clone(), DocItemKind::Macro);
                let (main_doc, doc_params, doc_returns) = self.process_doc(doc.clone());
                item.doc = main_doc;
                item.params = doc_params;
                item.returns = doc_returns;
                item.signature = format!("macro {}({})", name, ast_params.join(", "));
                Some(item)
            }
            StmtKind::Export(inner) => self.extract_stmt(inner),
            _ => None,
        }
    }

    fn extract_function(&self, func: &FunctionDecl) -> DocItem {
        let mut item = DocItem::new(func.name.clone(), DocItemKind::Function);
        let (main_doc, doc_params, doc_returns) = self.process_doc(func.doc.clone());
        item.doc = main_doc;
        item.params = doc_params;
        item.returns = doc_returns;
        item.visibility = func.visibility.clone();
        item.generics = func.generics.iter().map(format_generic).collect();
        item.signature = self.format_function_signature(
            &func.name,
            &func.generics,
            &func.params,
            func.return_ty.as_deref(),
        );
        if func.is_static {
            item.modifiers.push("static".to_string());
        }
        if func.is_abstract {
            item.modifiers.push("abstract".to_string());
        }
        if func.is_async {
            item.modifiers.push("async".to_string());
        }
        item
    }

    fn extract_field(&self, field: &FieldDecl) -> DocItem {
        let mut item = DocItem::new(field.name.clone(), DocItemKind::Field);
        let (main_doc, doc_params, doc_returns) = self.process_doc(field.doc.clone());
        item.doc = main_doc;
        item.params = doc_params;
        item.returns = doc_returns;
        item.visibility = field.visibility.clone();
        item.signature = if let Some(ty) = &field.ty {
            format!("{}: {}", field.name, ty)
        } else {
            field.name.clone()
        };
        if field.is_static {
            item.modifiers.push("static".to_string());
        }
        if field.is_const {
            item.modifiers.push("const".to_string());
        }
        item
    }

    fn extract_property(&self, prop: &PropertyDecl) -> DocItem {
        let mut item = DocItem::new(prop.name.clone(), DocItemKind::Property);
        let (main_doc, doc_params, doc_returns) = self.process_doc(prop.doc.clone());
        item.doc = main_doc;
        item.params = doc_params;
        item.returns = doc_returns;
        item.visibility = prop.visibility.clone();
        item.signature = if let Some(ty) = &prop.ty {
            format!("{}: {}", prop.name, ty)
        } else {
            prop.name.clone()
        };
        if prop.is_static {
            item.modifiers.push("static".to_string());
        }
        if prop.getter.is_some() {
            item.modifiers.push("get".to_string());
        }
        if prop.setter.is_some() {
            item.modifiers.push("set".to_string());
        }
        item
    }

    fn extract_enum(&self, enum_decl: &EnumDecl) -> DocItem {
        let mut item = DocItem::new(enum_decl.name.clone(), DocItemKind::Enum);
        let (main_doc, doc_params, doc_returns) = self.process_doc(enum_decl.doc.clone());
        item.doc = main_doc;
        item.params = doc_params;
        item.returns = doc_returns;
        item.signature = format!("enum {}", enum_decl.name);
        for variant in &enum_decl.variants {
            let mut variant_item = DocItem::new(variant.name.clone(), DocItemKind::Field);
            let (v_doc, v_params, v_returns) = self.process_doc(variant.doc.clone());
            variant_item.doc = v_doc;
            variant_item.params = v_params;
            variant_item.returns = v_returns;
            if let Some(params) = &variant.params {
                let params_str: Vec<String> = params
                    .iter()
                    .map(|p| {
                        if let Some(ty) = &p.ty {
                            format!("{}: {}", p.name, ty)
                        } else {
                            p.name.clone()
                        }
                    })
                    .collect();
                variant_item.signature = format!("{}({})", variant.name, params_str.join(", "));
            } else {
                variant_item.signature = variant.name.clone();
            }
            item.children.push(variant_item);
        }
        item
    }

    fn format_class_signature(
        &self,
        name: &str,
        generics: &[GenericParam],
        superclass: Option<&str>,
        implements: &[String],
        is_abstract: bool,
    ) -> String {
        let mut sig = String::new();
        if is_abstract {
            sig.push_str("abstract ");
        }
        sig.push_str("class ");
        sig.push_str(name);
        if !generics.is_empty() {
            sig.push('<');
            sig.push_str(
                &generics
                    .iter()
                    .map(format_generic)
                    .collect::<Vec<_>>()
                    .join(", "),
            );
            sig.push('>');
        }
        if let Some(super_name) = superclass {
            sig.push_str(" extends ");
            sig.push_str(super_name);
        }
        if !implements.is_empty() {
            sig.push_str(" implements ");
            sig.push_str(&implements.join(", "));
        }
        sig
    }

    fn format_interface_signature(
        &self,
        name: &str,
        generics: &[GenericParam],
        extends: &[String],
    ) -> String {
        let mut sig = String::new();
        sig.push_str("interface ");
        sig.push_str(name);
        if !generics.is_empty() {
            sig.push('<');
            sig.push_str(
                &generics
                    .iter()
                    .map(format_generic)
                    .collect::<Vec<_>>()
                    .join(", "),
            );
            sig.push('>');
        }
        if !extends.is_empty() {
            sig.push_str(" extends ");
            sig.push_str(&extends.join(", "));
        }
        sig
    }

    fn format_function_signature(
        &self,
        name: &str,
        generics: &[GenericParam],
        params: &[Param],
        return_ty: Option<&str>,
    ) -> String {
        let mut sig = String::new();
        sig.push_str("fn ");
        sig.push_str(name);
        if !generics.is_empty() {
            sig.push('<');
            sig.push_str(
                &generics
                    .iter()
                    .map(format_generic)
                    .collect::<Vec<_>>()
                    .join(", "),
            );
            sig.push('>');
        }
        sig.push('(');
        let params_str: Vec<String> = params
            .iter()
            .map(|p| {
                let mut s = String::new();
                if p.is_variadic {
                    s.push_str("...");
                }
                s.push_str(&p.name);
                if let Some(ty) = &p.ty {
                    s.push_str(": ");
                    s.push_str(ty);
                }
                s
            })
            .collect();
        sig.push_str(&params_str.join(", "));
        sig.push(')');
        if let Some(ret) = return_ty {
            sig.push_str(" -> ");
            sig.push_str(ret);
        }
        sig
    }
}

impl Default for DocGenerator {
    fn default() -> Self {
        Self::new()
    }
}

fn format_generic(g: &GenericParam) -> String {
    if g.constraints.is_empty() {
        g.name.clone()
    } else {
        format!("{}: {}", g.name, g.constraints.join(" + "))
    }
}

/// Renders documentation items to Markdown
pub struct MarkdownRenderer;

impl MarkdownRenderer {
    pub fn new() -> Self {
        Self
    }

    /// Render a list of doc items as a complete Markdown document
    pub fn render(&self, items: &[DocItem], title: &str) -> String {
        let mut output = String::new();

        // Title
        writeln!(output, "# {}\n", title).unwrap();

        // Table of contents
        if !items.is_empty() {
            writeln!(output, "## Contents\n").unwrap();
            for item in items {
                writeln!(output, "- [{}](#{})", item.name, self.anchor(&item.name)).unwrap();
            }
            writeln!(output).unwrap();
        }

        // Render each item
        for item in items {
            self.render_item(&mut output, item, 2);
        }

        output
    }

    fn render_item(&self, output: &mut String, item: &DocItem, level: usize) {
        let heading = "#".repeat(level);

        // Name and type
        writeln!(output, "{} {}\n", heading, item.name).unwrap();

        // Modifiers badge
        if !item.modifiers.is_empty() {
            let badges: Vec<String> = item.modifiers.iter().map(|m| format!("`{}`", m)).collect();
            writeln!(output, "{}\n", badges.join(" ")).unwrap();
        }

        // Signature
        writeln!(output, "```pleno\n{}\n```\n", item.signature).unwrap();

        // Documentation
        if let Some(doc) = &item.doc {
            writeln!(output, "{}\n", doc.trim()).unwrap();
        }

        // Parameters
        if !item.params.is_empty() {
            writeln!(output, "#### Parameters\n").unwrap();
            for param in &item.params {
                writeln!(output, "- `{}`: {}\n", param.name, param.description).unwrap();
            }
        }

        // Returns
        if let Some(ret) = &item.returns {
            writeln!(output, "#### Returns\n").unwrap();
            writeln!(output, "{}\n", ret.description).unwrap();
        }

        // Children (methods, fields, etc.)
        if !item.children.is_empty() {
            // Group children by kind
            let mut fields: Vec<&DocItem> = Vec::new();
            let mut properties: Vec<&DocItem> = Vec::new();
            let mut methods: Vec<&DocItem> = Vec::new();
            let mut others: Vec<&DocItem> = Vec::new();

            for child in &item.children {
                match child.kind {
                    DocItemKind::Field => fields.push(child),
                    DocItemKind::Property => properties.push(child),
                    DocItemKind::Method => methods.push(child),
                    _ => others.push(child),
                }
            }

            if !fields.is_empty() {
                writeln!(output, "{}# Fields\n", heading).unwrap();
                for field in fields {
                    self.render_member(output, field);
                }
            }

            if !properties.is_empty() {
                writeln!(output, "{}# Properties\n", heading).unwrap();
                for prop in properties {
                    self.render_member(output, prop);
                }
            }

            if !methods.is_empty() {
                writeln!(output, "{}# Methods\n", heading).unwrap();
                for method in methods {
                    self.render_member(output, method);
                }
            }

            if !others.is_empty() {
                for other in others {
                    self.render_item(output, other, level + 1);
                }
            }
        }

        writeln!(output, "---\n").unwrap();
    }

    fn render_member(&self, output: &mut String, item: &DocItem) {
        // Visibility badge
        let vis = match &item.visibility {
            Some(Visibility::Private) => "🔒 ",
            Some(Visibility::Protected) => "🔐 ",
            _ => "",
        };

        // Modifiers
        let mods: String = item.modifiers.iter().map(|m| format!("`{}` ", m)).collect();

        writeln!(output, "#### {}{}`{}`\n", vis, mods, item.signature).unwrap();

        if let Some(doc) = &item.doc {
            writeln!(output, "{}\n", doc.trim()).unwrap();
        }

        // Parameters
        if !item.params.is_empty() {
            for param in &item.params {
                writeln!(
                    output,
                    "- **param** `{}`: {}\n",
                    param.name, param.description
                )
                .unwrap();
            }
        }

        // Returns
        if let Some(ret) = &item.returns {
            writeln!(output, "- **returns** {}\n", ret.description).unwrap();
        }
    }

    fn anchor(&self, name: &str) -> String {
        name.to_lowercase().replace(' ', "-")
    }
}

impl Default for MarkdownRenderer {
    fn default() -> Self {
        Self::new()
    }
}

/// Generate documentation for a source file
pub fn generate_docs(source: &str, file_name: &str) -> Result<String, String> {
    // Lex
    let tokens = pleno_lexer::lex(source).map_err(|e| format!("Lex error: {}", e.message))?;

    // Parse
    let mut parser = pleno_parser::Parser::new(tokens, source);
    let program = parser
        .parse_program()
        .map_err(|e| format!("Parse error: {}", e.message))?;

    // Extract docs
    let generator = DocGenerator::new();
    let items = generator.extract(&program);

    // Render
    let renderer = MarkdownRenderer::new();
    let title = file_name.trim_end_matches(".pln");
    Ok(renderer.render(&items, title))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_docs_function() {
        let source = r#"
/// Adds two numbers together.
/// Returns the sum.
fn add(a, b) {
    return a + b;
}
"#;
        let result = generate_docs(source, "math.pln").unwrap();
        assert!(result.contains("add"));
        assert!(result.contains("Adds two numbers together"));
        assert!(result.contains("fn add(a, b)"));
    }

    #[test]
    fn test_generate_docs_class() {
        let source = r#"
/// A simple point in 2D space.
class Point {
    /// X coordinate
    x: Float;
    /// Y coordinate
    y: Float;
}
"#;
        let result = generate_docs(source, "geometry.pln").unwrap();
        assert!(result.contains("Point"));
        assert!(result.contains("A simple point in 2D space"));
        assert!(result.contains("X coordinate"));
        assert!(result.contains("Y coordinate"));
    }

    #[test]
    fn test_doc_item_creation() {
        let item = DocItem::new("test".to_string(), DocItemKind::Function);
        assert_eq!(item.name, "test");
        assert_eq!(item.kind, DocItemKind::Function);
        assert!(item.doc.is_none());
        assert!(item.children.is_empty());
    }

    #[test]
    fn test_generate_docs_with_tags() {
        let source = r#"
/// Adds two numbers together.
/// @param a The first number.
/// @param b The second number.
/// @returns The sum.
fn add(a, b) {
    return a + b;
}
"#;
        let result = generate_docs(source, "math.pln").unwrap();
        assert!(result.contains("add"));
        assert!(result.contains("Adds two numbers together"));
        assert!(result.contains("#### Parameters"));
        assert!(result.contains("- `a`: The first number."));
        assert!(result.contains("- `b`: The second number."));
        assert!(result.contains("#### Returns"));
        assert!(result.contains("The sum."));
    }
}
