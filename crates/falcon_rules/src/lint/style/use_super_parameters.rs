//! Flags a constructor parameter forwarded verbatim to `super(...)` and used
//! nowhere else (`use-super-parameters`, adopted from package:lints): such a
//! parameter should become a super parameter `super.x`.
//!
//! Conservative: the parameter must appear exactly once across the whole
//! constructor — as a plain `Ident` positional super argument, or as a named
//! super argument whose key and value both equal the parameter name.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::Program;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::Visitor;
use std::collections::HashMap;

pub struct UseSuperParameters;

impl Rule for UseSuperParameters {
    fn name(&self) -> &'static str {
        "use-super-parameters"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for decl in &program.declarations {
            if let Some(members) = members_of(decl) {
                for member in members {
                    if let ClassMember::Constructor(ctor) = member {
                        check_ctor(ctor, ctx, &mut diags);
                    }
                }
            }
        }
        diags
    }
}

fn members_of(decl: &TopLevelDecl) -> Option<&[ClassMember]> {
    match decl {
        TopLevelDecl::Class(c) => Some(&c.members),
        TopLevelDecl::MixinClass(mc) => Some(&mc.members),
        TopLevelDecl::Enum(e) => Some(&e.members),
        TopLevelDecl::ExtensionType(e) => Some(&e.members),
        _ => None,
    }
}

#[derive(Default)]
struct Counter {
    counts: HashMap<String, usize>,
}

impl Visitor for Counter {
    fn visit_identifier(&mut self, node: &Identifier) {
        *self.counts.entry(node.name.clone()).or_insert(0) += 1;
    }
}

fn check_ctor(ctor: &ConstructorDecl, ctx: &AnalyzeContext, diags: &mut Vec<Diagnostic>) {
    let Some(super_args) = ctor.initializers.iter().find_map(|init| match init {
        ConstructorInitializer::SuperCall { args, .. } => Some(args),
        _ => None,
    }) else {
        return;
    };

    // Count every identifier use across all initializers and the body. A
    // forwarded parameter used exactly once is used only by the super call.
    let mut counter = Counter::default();
    for init in &ctor.initializers {
        match init {
            ConstructorInitializer::SuperCall { args, .. }
            | ConstructorInitializer::ThisCall { args, .. } => {
                for a in &args.positional {
                    counter.visit_expr(a);
                }
                for n in &args.named {
                    counter.visit_expr(&n.value);
                }
            }
            ConstructorInitializer::FieldInit { value, .. } => counter.visit_expr(value),
            ConstructorInitializer::Assert {
                condition, message, ..
            } => {
                counter.visit_expr(condition);
                if let Some(m) = message {
                    counter.visit_expr(m);
                }
            }
        }
    }
    if let Some(FunctionBody::Block(block)) = &ctor.body {
        for stmt in &block.stmts {
            counter.visit_stmt(stmt);
        }
    } else if let Some(FunctionBody::Arrow(expr, _)) = &ctor.body {
        counter.visit_expr(expr);
    }

    let used_once = |name: &str| counter.counts.get(name) == Some(&1);

    // Positional params forward verbatim only when the super call passes the
    // same identifier at the same positional index (order-preserving).
    let positional: Vec<&FormalParam> = ctor
        .params
        .positional
        .iter()
        .chain(&ctor.params.optional_positional)
        .collect();
    for (i, param) in positional.iter().enumerate() {
        if param.is_field || param.is_super {
            continue;
        }
        if let Some(Expr::Ident(id)) = super_args.positional.get(i)
            && id.name == param.name.name
            && used_once(&param.name.name)
        {
            flag(diags, ctx, &param.name.span);
        }
    }

    // Named params forward verbatim only via `super(name: name)`.
    for param in &ctor.params.named {
        if param.is_field || param.is_super {
            continue;
        }
        let forwarded = super_args.named.iter().any(|n| {
            n.name.name == param.name.name
                && matches!(&n.value, Expr::Ident(id) if id.name == param.name.name)
        });
        if forwarded && used_once(&param.name.name) {
            flag(diags, ctx, &param.name.span);
        }
    }
}

fn flag(diags: &mut Vec<Diagnostic>, ctx: &AnalyzeContext, span: &Span) {
    diags.push(Diagnostic::new(
        "use-super-parameters",
        Severity::Warning,
        "Use a super parameter instead of forwarding a parameter to super.".to_string(),
        ctx.file_path.to_string_lossy().into_owned(),
        DiagSpan {
            start: span.start,
            end: span.end,
        },
    ));
}
