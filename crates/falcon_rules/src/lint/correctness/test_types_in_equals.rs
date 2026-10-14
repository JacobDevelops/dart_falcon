//! Flags casting the parameter of `operator ==` instead of testing its type.
//!
//! `==` must accept any object and answer `false` for unrelated types. Casting
//! the parameter with `as` throws a `CastError` for those instead, so comparing
//! with an object of another type crashes. Test the type with `is` (or a
//! pattern) and return `false` when it does not match.
//!
//! ```dart
//! class Field {
//!   @override
//!   bool operator ==(Object other) {
//!     Field otherField = other as Field;  // flagged
//!     return value == otherField.value;
//!   }
//! }
//! ```
//!
//! Adopted from the official `test_types_in_equals` lint.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr, walk_function_body};

#[derive(Default)]
pub struct TestTypesInEquals;

impl Rule for TestTypesInEquals {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for decl in &program.declarations {
            let Some(members) = crate::lint::declarations::members_of(decl) else {
                continue;
            };
            let type_name = match decl {
                TopLevelDecl::Class(node) => node.name.name.as_str(),
                TopLevelDecl::MixinClass(node) => node.name.name.as_str(),
                TopLevelDecl::Enum(node) => node.name.name.as_str(),
                TopLevelDecl::Mixin(node) => node.name.name.as_str(),
                TopLevelDecl::Extension(node) => {
                    let span = node.on_type.span();
                    ctx.source.get(span.start..span.end).unwrap_or("unknown")
                }
                _ => "unknown",
            };
            for member in members {
                let ClassMember::Operator(op) = member else {
                    continue;
                };
                let (Some(body), [param]) = (&op.body, op.params.params.as_slice()) else {
                    continue;
                };
                if op.op != "==" {
                    continue;
                }
                let mut finder = ParameterCasts {
                    name: &param.name.name,
                    spans: Vec::new(),
                };
                walk_function_body(&mut finder, body);
                diags.extend(finder.spans.into_iter().map(|span| {
                    ctx.diagnostic(
                        "test-types-in-equals",
                        format!("Missing type test for '{type_name}' in '=='."),
                        span,
                    )
                }));
            }
        }
        diags
    }
}

/// Collects `<parameter> as T` casts.
struct ParameterCasts<'a> {
    name: &'a str,
    spans: Vec<Span>,
}

impl Visitor for ParameterCasts<'_> {
    fn visit_expr(&mut self, node: &Expr) {
        if let Expr::As { expr, span, .. } = node
            && matches!(&**expr, Expr::Ident(identifier) if identifier.name == self.name)
        {
            self.spans.push(*span);
        }
        walk_expr(self, node);
    }
}
