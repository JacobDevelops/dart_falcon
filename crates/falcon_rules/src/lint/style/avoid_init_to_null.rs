//! Flags explicit `= null` initializers on declarations that already default to
//! null. Ported from package:lints' `avoid_init_to_null`.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_field_decl, walk_stmt, walk_top_level_decl};

pub struct AvoidInitToNull;

impl Rule for AvoidInitToNull {
    fn name(&self) -> &'static str {
        "avoid-init-to-null"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut collector = Collector {
            diags: Vec::new(),
            file: ctx.file_path.to_string_lossy().into_owned(),
        };
        collector.visit_program(program);
        collector.diags
    }
}

struct Collector {
    diags: Vec<Diagnostic>,
    file: String,
}

impl Collector {
    /// A declaration only defaults to null when it is neither `final`, `const`
    /// nor `late` and its type is nullable (or absent/`dynamic`). In that case
    /// the `= null` is redundant and can be dropped without changing behaviour.
    fn check(
        &mut self,
        is_final: bool,
        is_const: bool,
        is_late: bool,
        var_type: &Option<DartType>,
        declarators: &[VarDeclarator],
    ) {
        if is_final || is_const || is_late || !defaults_to_null(var_type) {
            return;
        }
        for d in declarators {
            if matches!(&d.initializer, Some(Expr::NullLit { .. })) {
                self.diags.push(Diagnostic::new(
                    "avoid-init-to-null",
                    Severity::Warning,
                    "Don't explicitly initialize variables to null.",
                    self.file.clone(),
                    DiagSpan {
                        start: d.span.start,
                        end: d.span.end,
                    },
                ));
            }
        }
    }
}

fn defaults_to_null(var_type: &Option<DartType>) -> bool {
    match var_type {
        None | Some(DartType::Dynamic { .. }) => true,
        Some(t) => t.is_nullable(),
    }
}

impl Visitor for Collector {
    fn visit_top_level_decl(&mut self, node: &TopLevelDecl) {
        if let TopLevelDecl::Variable(v) = node {
            self.check(
                v.is_final,
                v.is_const,
                v.is_late,
                &v.var_type,
                &v.declarators,
            );
        }
        walk_top_level_decl(self, node);
    }

    fn visit_field_decl(&mut self, node: &FieldDecl) {
        self.check(
            node.is_final,
            node.is_const,
            node.is_late,
            &node.field_type,
            &node.declarators,
        );
        walk_field_decl(self, node);
    }

    fn visit_stmt(&mut self, node: &Stmt) {
        if let Stmt::LocalVar(lv) = node {
            self.check(
                lv.is_final,
                lv.is_const,
                lv.is_late,
                &lv.var_type,
                &lv.declarators,
            );
        }
        walk_stmt(self, node);
    }
}
