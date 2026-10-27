use pleno_ast::visitor::{
    Visitor, walk_enum_decl, walk_enum_variant, walk_expr, walk_field_decl, walk_function_decl,
    walk_param, walk_pattern, walk_property_decl, walk_stmt,
};
use pleno_ast::*;

pub enum Node<'a> {
    Stmt(&'a Stmt),
    Expr(&'a Expr),
    Pattern(&'a Pattern),
    Param(&'a Param),
    Field(&'a FieldDecl),
    Function(&'a FunctionDecl),
    #[allow(dead_code)]
    Property(&'a PropertyDecl),
    #[allow(dead_code)]
    Enum(&'a EnumDecl),
    EnumVariant(&'a EnumVariant),
}

pub struct NodeFinder<'a> {
    offset: usize,
    pub innermost_node: Option<Node<'a>>,
}

impl<'a> NodeFinder<'a> {
    pub fn new(offset: usize) -> Self {
        Self {
            offset,
            innermost_node: None,
        }
    }
}

impl<'ast> Visitor<'ast> for NodeFinder<'ast> {
    fn visit_stmt(&mut self, stmt: &'ast Stmt) {
        if stmt.span.contains(self.offset) {
            self.innermost_node = Some(Node::Stmt(stmt));
            walk_stmt(self, stmt);
        }
    }

    fn visit_expr(&mut self, expr: &'ast Expr) {
        if expr.span.contains(self.offset) {
            self.innermost_node = Some(Node::Expr(expr));
            walk_expr(self, expr);
        }
    }

    fn visit_param(&mut self, param: &'ast Param) {
        if param.span.contains(self.offset) {
            self.innermost_node = Some(Node::Param(param));
            walk_param(self, param);
        }
    }

    fn visit_field_decl(&mut self, field: &'ast FieldDecl) {
        if field.span.contains(self.offset) {
            self.innermost_node = Some(Node::Field(field));
            walk_field_decl(self, field);
        }
    }

    fn visit_function_decl(&mut self, decl: &'ast FunctionDecl) {
        if decl.span.contains(self.offset) {
            self.innermost_node = Some(Node::Function(decl));
            walk_function_decl(self, decl);
        }
    }

    fn visit_property_decl(&mut self, decl: &'ast PropertyDecl) {
        if decl.span.contains(self.offset) {
            self.innermost_node = Some(Node::Property(decl));
            walk_property_decl(self, decl);
        }
    }

    fn visit_pattern(&mut self, pattern: &'ast Pattern) {
        if pattern.span.contains(self.offset) {
            self.innermost_node = Some(Node::Pattern(pattern));
            walk_pattern(self, pattern);
        }
    }

    fn visit_enum_decl(&mut self, decl: &'ast EnumDecl) {
        if decl.span.contains(self.offset) {
            self.innermost_node = Some(Node::Enum(decl));
            walk_enum_decl(self, decl);
        }
    }

    fn visit_enum_variant(&mut self, variant: &'ast EnumVariant) {
        if variant.span.contains(self.offset) {
            self.innermost_node = Some(Node::EnumVariant(variant));
            walk_enum_variant(self, variant);
        }
    }
}

pub fn find_node_at<'a>(program: &'a Program, offset: usize) -> Option<Node<'a>> {
    let mut finder = NodeFinder::new(offset);
    finder.visit_program(program);
    finder.innermost_node
}
