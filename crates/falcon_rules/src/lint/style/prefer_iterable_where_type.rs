//! Flags `.where((e) => e is T)`, which `.whereType<T>()` expresses directly.
//! Adopted from package:lints `prefer_iterable_whereType`.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr};

pub struct PreferIterableWhereType;

impl Rule for PreferIterableWhereType {
    fn name(&self) -> &'static str {
        "prefer-iterable-where-type"
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
    fn push(&mut self, span: &Span) {
        self.diags.push(Diagnostic::new(
            "prefer-iterable-where-type",
            Severity::Warning,
            "Use 'whereType<T>()' instead of 'where' with an 'is' check.",
            self.file.clone(),
            DiagSpan {
                start: span.start,
                end: span.end,
            },
        ));
    }
}

impl Visitor for Collector {
    fn visit_expr(&mut self, node: &Expr) {
        if let Expr::Call { callee, args, .. } = node
            && let Expr::Field { field, .. } = &**callee
            && field.name == "where"
            && args.positional.len() == 1
            && args.named.is_empty()
            && is_is_check_closure(&args.positional[0])
        {
            self.push(&field.span);
        }
        walk_expr(self, node);
    }
}

/// True for a single-parameter arrow closure whose body is `param is T` (a
/// non-negated type test on the parameter itself).
fn is_is_check_closure(expr: &Expr) -> bool {
    let Expr::FuncExpr { params, body, .. } = expr else {
        return false;
    };
    if params.positional.len() != 1
        || !params.optional_positional.is_empty()
        || !params.named.is_empty()
    {
        return false;
    }
    let param_name = &params.positional[0].name.name;
    if let FunctionBody::Arrow(inner, _) = &**body
        && let Expr::Is {
            expr: tested,
            negated: false,
            ..
        } = &**inner
        && let Expr::Ident(id) = &**tested
    {
        return &id.name == param_name;
    }
    false
}
