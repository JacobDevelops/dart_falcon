//! Flags an `as` cast to a type the operand already has.
//!
//! Casting a variable to its own declared type does nothing at runtime and
//! hides the fact that no conversion occurs; remove it. Lacking full type
//! inference, the rule tracks the non-nullable declared types of local
//! variables and class fields and reports `x as T` when `x`'s declared type
//! matches `T` (same name and type arguments). A nullable-declared operand is
//! never flagged, since a cast can still strip its nullability.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use std::collections::HashMap;

pub struct AvoidUnnecessaryTypeCasts;

impl AvoidUnnecessaryTypeCasts {
    fn collect_local_vars(&self, stmts: &[Stmt]) -> HashMap<String, DartType> {
        let mut var_types = HashMap::new();

        for stmt in stmts {
            match stmt {
                Stmt::LocalVar(LocalVarDecl {
                    var_type,
                    declarators,
                    ..
                }) => {
                    if let Some(var_t) = var_type
                        && let DartType::Named(named) = var_t
                        && !named.is_nullable
                    {
                        for declarator in declarators {
                            var_types.insert(declarator.name.name.clone(), var_t.clone());
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

    fn collect_class_fields(class: &ClassDecl) -> HashMap<String, DartType> {
        let mut map = HashMap::new();
        for member in &class.members {
            if let ClassMember::Field(field) = member
                && let Some(field_type) = &field.field_type
                && let DartType::Named(named) = field_type
                && !named.is_nullable
            {
                for declarator in &field.declarators {
                    map.insert(declarator.name.name.clone(), field_type.clone());
                }
            }
        }
        map
    }

    fn types_match(declared: &DartType, cast_type: &DartType) -> bool {
        if let (DartType::Named(decl), DartType::Named(cast)) = (declared, cast_type) {
            if decl.is_nullable {
                return false;
            }
            let decl_name = decl.segments.first().map(|s| s.name.as_str());
            let cast_name = cast.segments.first().map(|s| s.name.as_str());
            if decl_name != cast_name {
                return false;
            }
            if cast.type_args.len() != decl.type_args.len() {
                return false;
            }
            if cast.type_args.is_empty() {
                return true;
            }
            cast.type_args
                .iter()
                .zip(decl.type_args.iter())
                .all(|(c, d)| {
                    if let (DartType::Named(cn), DartType::Named(dn)) = (c, d) {
                        cn.segments.first().map(|s| s.name.as_str())
                            == dn.segments.first().map(|s| s.name.as_str())
                    } else {
                        false
                    }
                })
        } else {
            false
        }
    }

    fn check_as_expr(
        &self,
        expr: &Expr,
        dart_type: &DartType,
        var_types: &HashMap<String, DartType>,
    ) -> bool {
        if let Expr::Ident(Identifier { name, .. }) = expr
            && let Some(declared) = var_types.get(name)
        {
            return Self::types_match(declared, dart_type);
        }
        false
    }
}

impl Rule for AvoidUnnecessaryTypeCasts {
    fn name(&self) -> &'static str {
        "avoid-unnecessary-type-casts"
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
                    let class_fields = Self::collect_class_fields(class_decl);
                    for member in &class_decl.members {
                        let body = match member {
                            ClassMember::Method(m) => m.body.as_ref(),
                            ClassMember::Getter(g) => g.body.as_ref(),
                            _ => None,
                        };
                        if let Some(FunctionBody::Block(block)) = body {
                            let mut var_types = class_fields.clone();
                            var_types.extend(self.collect_local_vars(&block.stmts));
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

impl AvoidUnnecessaryTypeCasts {
    /// Report every redundant `as` cast anywhere in `block`. Traversal goes
    /// through the exhaustive shared walker, so a cast cannot hide inside newer
    /// syntax (a record-pattern declaration, a switch expression, a labeled
    /// statement) the way a hand-rolled `_ => {}` walk allowed.
    fn check_block(
        &self,
        block: &Block,
        var_types: &HashMap<String, DartType>,
        ctx: &AnalyzeContext,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        falcon_syntax::visitor::for_each_expr_in_stmts(&block.stmts, &mut |e| {
            if let Expr::As {
                expr,
                dart_type,
                span,
            } = e
                && self.check_as_expr(expr, dart_type, var_types)
            {
                diagnostics.push(Diagnostic::new(
                    "avoid-unnecessary-type-casts",
                    Severity::Warning,
                    "Unnecessary type cast — variable is already known to be this type",
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
