//! Flags a redundant `this.` qualifier on member access (`unnecessary-this`,
//! adopted from package:lints). `this.x` / `this.m()` is only needed when a
//! parameter or local in scope shadows the member name; otherwise the `this.`
//! is noise.
//!
//! Conservative: the shadow set is over-approximated (every parameter, local
//! variable, loop/catch/pattern binding, and closure parameter anywhere in the
//! member is treated as an in-scope shadow), so a name that could plausibly be
//! shadowed is never flagged. Constructor initializer lists are not visited, so
//! the `this.` required there is never touched.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::Program;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{self, Visitor};
use std::collections::HashSet;

pub struct UnnecessaryThis;

impl Rule for UnnecessaryThis {
    fn name(&self) -> &'static str {
        "unnecessary-this"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for decl in &program.declarations {
            if let Some(members) = members_of(decl) {
                for member in members {
                    check_member(member, ctx, &mut diags);
                }
            }
        }
        diags
    }
}

fn members_of(decl: &TopLevelDecl) -> Option<&[ClassMember]> {
    match decl {
        TopLevelDecl::Class(c) => Some(&c.members),
        TopLevelDecl::Mixin(m) => Some(&m.members),
        TopLevelDecl::MixinClass(mc) => Some(&mc.members),
        TopLevelDecl::Enum(e) => Some(&e.members),
        TopLevelDecl::Extension(e) => Some(&e.members),
        TopLevelDecl::ExtensionType(e) => Some(&e.members),
        _ => None,
    }
}

fn check_member(member: &ClassMember, ctx: &AnalyzeContext, diags: &mut Vec<Diagnostic>) {
    let (params, body): (Vec<&str>, &Option<FunctionBody>) = match member {
        ClassMember::Method(m) if !m.is_static => (param_names(&m.params), &m.body),
        ClassMember::Getter(g) if !g.is_static => (Vec::new(), &g.body),
        ClassMember::Setter(s) if !s.is_static => (vec![s.param.name.as_str()], &s.body),
        ClassMember::Operator(o) => (param_names(&o.params), &o.body),
        // Only the constructor body is analyzed; initializers keep their `this.`.
        ClassMember::Constructor(c) => (param_names(&c.params), &c.body),
        _ => return,
    };
    let Some(body) = body else {
        return;
    };

    let mut shadows = Shadows {
        names: HashSet::new(),
    };
    for p in &params {
        shadows.names.insert((*p).to_string());
    }
    collect_body(&mut shadows, body);

    let mut finder = ThisFinder {
        ctx,
        shadows: &shadows.names,
        diags,
    };
    walk_body(&mut finder, body);
}

fn param_names(params: &FormalParamList) -> Vec<&str> {
    params
        .positional
        .iter()
        .chain(&params.optional_positional)
        .chain(&params.named)
        .map(|p| p.name.name.as_str())
        .collect()
}

fn walk_body<V: Visitor>(v: &mut V, body: &FunctionBody) {
    match body {
        FunctionBody::Block(block) => {
            for stmt in &block.stmts {
                v.visit_stmt(stmt);
            }
        }
        FunctionBody::Arrow(expr, _) => v.visit_expr(expr),
        FunctionBody::Native(_, _) => {}
    }
}

struct Shadows {
    names: HashSet<String>,
}

fn collect_body(shadows: &mut Shadows, body: &FunctionBody) {
    walk_body(shadows, body);
}

impl Visitor for Shadows {
    fn visit_stmt(&mut self, node: &Stmt) {
        match node {
            Stmt::LocalVar(local) => {
                for d in &local.declarators {
                    self.names.insert(d.name.name.clone());
                }
            }
            Stmt::LocalFunc(func) => {
                self.names.insert(func.name.name.clone());
            }
            Stmt::For(for_stmt) => {
                if let Some(ForInit::ForIn { name, .. }) = &for_stmt.init {
                    self.names.insert(name.name.clone());
                }
            }
            Stmt::TryCatch(tc) => {
                for catch in &tc.catches {
                    if let Some(e) = &catch.exception_var {
                        self.names.insert(e.name.clone());
                    }
                    if let Some(s) = &catch.stack_trace_var {
                        self.names.insert(s.name.clone());
                    }
                }
            }
            _ => {}
        }
        visitor::walk_stmt(self, node);
    }

    fn visit_formal_param(&mut self, node: &FormalParam) {
        self.names.insert(node.name.name.clone());
        visitor::walk_formal_param(self, node);
    }

    fn visit_pattern(&mut self, node: &Pattern) {
        if let Pattern::Variable { name, .. } = node {
            self.names.insert(name.name.clone());
        }
        visitor::walk_pattern(self, node);
    }
}

struct ThisFinder<'a, 'c> {
    ctx: &'a AnalyzeContext<'c>,
    shadows: &'a HashSet<String>,
    diags: &'a mut Vec<Diagnostic>,
}

impl Visitor for ThisFinder<'_, '_> {
    fn visit_expr(&mut self, node: &Expr) {
        if let Expr::Field { object, field, .. } = node
            && matches!(object.as_ref(), Expr::This { .. })
            && !self.shadows.contains(&field.name)
        {
            let span = object.span();
            self.diags.push(Diagnostic::new(
                "unnecessary-this",
                Severity::Warning,
                "Unnecessary 'this.' qualifier.".to_string(),
                self.ctx.file_path.to_string_lossy().into_owned(),
                DiagSpan {
                    start: span.start,
                    end: span.end,
                },
            ));
        }
        visitor::walk_expr(self, node);
    }
}
