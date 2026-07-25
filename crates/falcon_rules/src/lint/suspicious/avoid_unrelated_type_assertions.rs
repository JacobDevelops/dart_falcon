//! Flags an `is` check that can never succeed because the operand's type is
//! unrelated to the tested type.
//!
//! A test like `'text' is int` or `42 is String` is always false, so the guarded
//! branch is dead code — usually a leftover from a refactor or a paste that
//! carried the wrong type. Detection works from literal operands and locally
//! declared variables initialized from literals, comparing the operand's category
//! (`String`, `int`, `double`, `bool`, `List`, `Map`, `Set`) against the tested
//! type and reporting known-incompatible pairings. Correct the test to the type
//! you actually mean, or remove the unreachable branch.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use std::collections::HashMap;

pub struct AvoidUnrelatedTypeAssertions;

impl AvoidUnrelatedTypeAssertions {
    fn is_incompatible_type(&self, literal_category: &str, type_name: &str) -> bool {
        match literal_category {
            "String" => matches!(
                type_name,
                "int" | "double" | "bool" | "List" | "Map" | "Set"
            ),
            "int" => matches!(
                type_name,
                "String" | "bool" | "List" | "Map" | "Set" | "double"
            ),
            "double" => matches!(
                type_name,
                "String" | "bool" | "List" | "Map" | "Set" | "int"
            ),
            "bool" => matches!(
                type_name,
                "String" | "int" | "double" | "List" | "Map" | "Set"
            ),
            "List" => matches!(type_name, "String" | "int" | "double" | "bool" | "Map"),
            "Map" => matches!(type_name, "String" | "int" | "double" | "bool" | "List"),
            "Set" => matches!(
                type_name,
                "String" | "int" | "double" | "bool" | "List" | "Map"
            ),
            _ => false,
        }
    }

    fn get_literal_category(expr: &Expr) -> Option<&'static str> {
        match expr {
            Expr::StringLit(_) => Some("String"),
            Expr::IntLit { .. } => Some("int"),
            Expr::DoubleLit { .. } => Some("double"),
            Expr::BoolLit { .. } => Some("bool"),
            Expr::List { .. } => Some("List"),
            Expr::Map { .. } => Some("Map"),
            Expr::Set { .. } => Some("Set"),
            _ => None,
        }
    }

    fn get_first_segment(dart_type: &DartType) -> Option<String> {
        match dart_type {
            DartType::Named(named) => named.segments.first().map(|first| first.name.clone()),
            _ => None,
        }
    }

    fn infer_type_from_expr(expr: &Expr) -> Option<String> {
        match expr {
            Expr::StringLit(_) => Some("String".to_string()),
            Expr::IntLit { .. } => Some("int".to_string()),
            Expr::DoubleLit { .. } => Some("double".to_string()),
            Expr::BoolLit { .. } => Some("bool".to_string()),
            Expr::List { .. } => Some("List".to_string()),
            Expr::Map { .. } => Some("Map".to_string()),
            Expr::Set { .. } => Some("Set".to_string()),
            _ => None,
        }
    }

    fn collect_local_vars(&self, stmts: &[Stmt]) -> HashMap<String, String> {
        let mut var_types = HashMap::new();

        for stmt in stmts {
            match stmt {
                Stmt::LocalVar(LocalVarDecl {
                    var_type,
                    declarators,
                    ..
                }) => {
                    for declarator in declarators {
                        if let Some(init_expr) = &declarator.initializer
                            && let Some(inferred) = Self::infer_type_from_expr(init_expr)
                        {
                            var_types.insert(declarator.name.name.clone(), inferred);
                        }

                        if let Some(var_t) = var_type
                            && let Some(type_name) = Self::get_first_segment(var_t)
                        {
                            var_types.insert(declarator.name.name.clone(), type_name);
                        }
                    }
                }
                Stmt::If(IfStmt {
                    then_branch,
                    else_branch,
                    ..
                }) => {
                    if let Stmt::Block(Block {
                        stmts: then_stmts, ..
                    }) = then_branch.as_ref()
                    {
                        let nested = self.collect_local_vars(then_stmts);
                        var_types.extend(nested);
                    }

                    if let Some(else_stmt) = else_branch
                        && let Stmt::Block(Block {
                            stmts: else_stmts, ..
                        }) = else_stmt.as_ref()
                    {
                        let nested = self.collect_local_vars(else_stmts);
                        var_types.extend(nested);
                    }
                }
                Stmt::Block(Block { stmts, .. }) => {
                    let nested = self.collect_local_vars(stmts);
                    var_types.extend(nested);
                }
                _ => {}
            }
        }

        var_types
    }

    fn check_is_expr(
        &self,
        expr: &Expr,
        dart_type: &DartType,
        var_types: &HashMap<String, String>,
    ) -> bool {
        if let Some(category) = Self::get_literal_category(expr)
            && let Some(type_name) = Self::get_first_segment(dart_type)
        {
            return self.is_incompatible_type(category, &type_name);
        }

        if let Expr::Ident(Identifier { name, .. }) = expr
            && let Some(var_type) = var_types.get(name)
            && let Some(assert_type) = Self::get_first_segment(dart_type)
        {
            return self.is_incompatible_type(var_type, &assert_type);
        }

        false
    }
}

impl Rule for AvoidUnrelatedTypeAssertions {
    fn name(&self) -> &'static str {
        "avoid-unrelated-type-assertions"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();

        for decl in &program.declarations {
            match decl {
                TopLevelDecl::Function(func_decl) => {
                    if let Some(FunctionBody::Block(block)) = &func_decl.body {
                        let var_types = self.collect_local_vars(&block.stmts);
                        self.check_block(block, &var_types, ctx, &mut diagnostics);
                    }
                }
                TopLevelDecl::Class(class_decl) => {
                    for member in &class_decl.members {
                        let body = match member {
                            ClassMember::Method(m) => m.body.as_ref(),
                            ClassMember::Getter(g) => g.body.as_ref(),
                            _ => None,
                        };
                        if let Some(FunctionBody::Block(block)) = body {
                            let var_types = self.collect_local_vars(&block.stmts);
                            self.check_block(block, &var_types, ctx, &mut diagnostics);
                        }
                    }
                }
                _ => {}
            }
        }

        diagnostics
    }
}

impl AvoidUnrelatedTypeAssertions {
    /// Report every impossible `is` test anywhere in `block`. Traversal goes
    /// through the exhaustive shared walker, so an assertion cannot hide inside
    /// a labeled statement or a record-pattern declaration the way the previous
    /// hand-picked statement list allowed.
    fn check_block(
        &self,
        block: &Block,
        var_types: &HashMap<String, String>,
        ctx: &AnalyzeContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        falcon_syntax::visitor::for_each_expr_in_stmts(&block.stmts, &mut |e| {
            if let Expr::Is {
                expr,
                dart_type,
                negated: false,
                span,
            } = e
                && self.check_is_expr(expr, dart_type, var_types)
            {
                diagnostics.push(Diagnostic::new(
                    "avoid-unrelated-type-assertions",
                    Severity::Warning,
                    "Type assertion can never be true — types are unrelated",
                    ctx.file_path.to_string_lossy().into_owned(),
                    DiagSpan {
                        start: span.start,
                        end: span.end,
                    },
                ));
            }
        });
    }
}
