//! Flags repeated property/getter chains in local variable initializers.
//!
//! Chains rooted in an identifier, `this`, or `super`, including receiver-based
//! method calls, are candidates. Receiverless calls, constructors and fresh
//! collections retain their separate evaluations. Initializer source text is
//! compared within each statement list. Mutation and deferred late reads reset
//! counts so a shared variable does not retain a stale value.
//! Unresolved `Type.member` calls remain candidates because constructors and static methods are ambiguous.
//!
//! ## Options
//!
//! `allowed_duplicated_chains` (integer, default: 2) — the occurrence index at
//! which a repeat is flagged; `2` flags the 2nd and later occurrences, `3` the
//! 3rd and later. Values below `2` are clamped to `2`.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, for_each_stmt_list};

#[derive(Default)]
pub struct PreferMovingToVariable;

impl Rule for PreferMovingToVariable {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        let threshold = allowed_duplicated_chains(ctx);
        let mut late = LateDeclarations {
            source: ctx.source,
            bindings: Vec::new(),
            has_deferred_effects: false,
            scope: Span {
                start: 0,
                end: ctx.source.len(),
            },
        };
        late.visit_program(program);
        if late.has_deferred_effects {
            // Statement lists also define lexical regions for cases and try/catch/finally bodies.
            for_each_stmt_list(program, &mut |stmts, _| {
                let (Some(first), Some(last)) = (stmts.first(), stmts.last()) else {
                    return;
                };
                let scope = Span {
                    start: first.span().start,
                    end: last.span().end,
                };
                for binding in &mut late.bindings {
                    if scope.start <= binding.start
                        && binding.start < scope.end
                        && binding.scope.start <= scope.start
                        && scope.end <= binding.scope.end
                    {
                        binding.scope = scope;
                    }
                }
            });
        }
        let model = ctx.identities.map(|identities| {
            falcon_analyze::SemanticModel::new(ctx.file_path, identities, ctx.types)
        });
        for_each_stmt_list(program, &mut |stmts, _| {
            check_stmts(stmts, &mut diags, ctx, threshold, &late, model.as_ref());
        });
        diags
    }
}

fn allowed_duplicated_chains(ctx: &AnalyzeContext) -> usize {
    crate::lint::options::usize(
        crate::lint::options::for_rule(ctx, "prefer-moving-to-variable"),
        "allowed_duplicated_chains",
    )
    .unwrap_or(2)
    .max(2)
}

use crate::lint::expressions::expr_src;

fn check_stmts(
    stmts: &[Stmt],
    diags: &mut Vec<Diagnostic>,
    ctx: &AnalyzeContext,
    threshold: usize,
    late: &LateDeclarations<'_>,
    model: Option<&falcon_analyze::SemanticModel<'_>>,
) {
    let mut counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();

    for stmt in stmts {
        let Stmt::LocalVar(lv) = stmt else {
            counts.clear();
            continue;
        };
        if lv.is_late {
            counts.clear();
            continue;
        }
        for decl in &lv.declarators {
            let Some(init) = &decl.initializer else {
                continue;
            };
            let mut reads = LateReads { late, found: false };
            if late.has_deferred_effects {
                reads.visit_expr(init);
            }
            if reads.found {
                counts.clear();
                continue;
            }
            let eligible = match init {
                Expr::Field { .. } => true,
                Expr::Call { callee, .. } => matches!(callee.as_ref(), Expr::Field { .. }),
                _ => false,
            };
            let calls = eligible
                .then(|| chain_calls(init, model, ctx.signatures))
                .flatten();
            if let Some(has_call) = calls {
                let src = expr_src(init, ctx.source);
                if has_call {
                    counts.retain(|previous, _| *previous == src);
                }
                let occurrence = *counts.entry(src).and_modify(|c| *c += 1).or_insert(1);
                if occurrence >= threshold {
                    diags.push(ctx.diagnostic(
                        "prefer-moving-to-variable",
                        "Duplicate expression — extract to a shared variable",
                        *init.span(),
                    ));
                }
            } else if !matches!(
                init,
                Expr::IntLit { .. }
                    | Expr::DoubleLit { .. }
                    | Expr::BoolLit { .. }
                    | Expr::NullLit { .. }
                    | Expr::Ident(_)
                    | Expr::This { .. }
                    | Expr::Super { .. }
                    | Expr::FuncExpr { .. }
            ) && !matches!(init, Expr::StringLit(literal) if literal.interpolations.is_empty())
            {
                counts.clear();
            }
        }
    }
}

struct LateReads<'a, 'source> {
    late: &'a LateDeclarations<'source>,
    found: bool,
}

impl Visitor for LateReads<'_, '_> {
    fn visit_identifier(&mut self, node: &Identifier) {
        self.found |= self
            .late
            .bindings
            .iter()
            .filter(|binding| {
                binding.name == node.name
                    && binding.scope.start <= node.span.start
                    && node.span.end <= binding.scope.end
                    && binding.start <= node.span.start
            })
            .max_by_key(|binding| (binding.scope.start, binding.start))
            .is_some_and(|binding| binding.deferred_effects);
    }

    fn visit_expr(&mut self, node: &Expr) {
        if !matches!(node, Expr::FuncExpr { .. }) {
            falcon_syntax::visitor::walk_expr(self, node);
        }
    }
}

fn chain_calls(
    expr: &Expr,
    model: Option<&falcon_analyze::SemanticModel<'_>>,
    signatures: Option<&falcon_analyze::SignatureIndex>,
) -> Option<bool> {
    let mut probe = ChainProbe {
        model,
        signatures,
        next: Some(*expr.span()),
        valid: true,
        has_call: false,
    };
    probe.visit_expr(expr);
    probe.valid.then_some(probe.has_call)
}

struct ChainProbe<'a, 'model> {
    model: Option<&'a falcon_analyze::SemanticModel<'model>>,
    signatures: Option<&'a falcon_analyze::SignatureIndex>,
    next: Option<Span>,
    valid: bool,
    has_call: bool,
}

impl Visitor for ChainProbe<'_, '_> {
    fn visit_expr(&mut self, node: &Expr) {
        // Follow only the receiver/callee spine, never argument expressions.
        if self.next != Some(*node.span()) || !self.valid {
            return;
        }
        self.next = match node {
            Expr::Field { object, .. } => Some(*object.span()),
            Expr::GenericInstantiation { target, .. } => Some(*target.span()),
            Expr::Call { callee, .. } => {
                self.has_call = true;
                self.valid = matches!(callee.as_ref(), Expr::Field { field, .. } if field.name != "new")
                    && !self.model.is_some_and(|model| {
                        falcon_analyze::resolve_call_constructor(callee, model, true).is_some_and(
                            |(identity, name)| {
                                !self.signatures.is_some_and(|signatures| {
                                    signatures.member(&identity, name).is_some()
                                        && signatures.constructor(&identity, name).is_none()
                                })
                            },
                        )
                    });
                Some(*callee.span())
            }
            Expr::Ident(_) | Expr::This { .. } | Expr::Super { .. } => None,
            _ => {
                self.valid = false;
                None
            }
        };
        falcon_syntax::visitor::walk_expr(self, node);
    }
}

struct LateBinding<'a> {
    name: &'a str,
    scope: Span,
    start: usize,
    deferred_effects: bool,
}

struct LateDeclarations<'a> {
    source: &'a str,
    bindings: Vec<LateBinding<'a>>,
    has_deferred_effects: bool,
    scope: Span,
}

impl LateDeclarations<'_> {
    fn enter_function(&mut self, span: Span) -> Span {
        let previous = self.scope;
        self.scope = span;
        previous
    }

    fn restore_function(&mut self, previous: Span) {
        self.scope = previous;
    }
}

impl Visitor for LateDeclarations<'_> {
    fn visit_function_decl(&mut self, node: &FunctionDecl) {
        let previous = self.enter_function(node.span);
        falcon_syntax::visitor::walk_function_decl(self, node);
        self.restore_function(previous);
    }

    fn visit_class_member(&mut self, node: &ClassMember) {
        let previous = self.enter_function(*node.span());
        falcon_syntax::visitor::walk_class_member(self, node);
        self.restore_function(previous);
    }

    fn visit_stmt(&mut self, node: &Stmt) {
        if let Stmt::LocalFunc(function) = node {
            let previous = self.enter_function(function.span);
            falcon_syntax::visitor::walk_stmt(self, node);
            self.restore_function(previous);
        } else {
            let previous = self.scope;
            if let Stmt::Block(block) = node {
                self.scope = block.span;
            }
            falcon_syntax::visitor::walk_stmt(self, node);
            self.scope = previous;
        }
    }

    fn visit_expr(&mut self, node: &Expr) {
        if matches!(node, Expr::FuncExpr { .. }) {
            let previous = self.enter_function(*node.span());
            falcon_syntax::visitor::walk_expr(self, node);
            self.restore_function(previous);
        } else {
            falcon_syntax::visitor::walk_expr(self, node);
        }
    }

    fn visit_local_var(&mut self, node: &LocalVarDecl) {
        for declaration in &node.declarators {
            let span = declaration.name.span;
            let deferred_effects = node.is_late && declaration.initializer.as_ref()
                .is_some_and(|expr| {
                    !matches!(expr, Expr::IntLit { .. } | Expr::DoubleLit { .. }
                        | Expr::BoolLit { .. } | Expr::NullLit { .. } | Expr::FuncExpr { .. })
                        && !matches!(expr, Expr::StringLit(literal) if literal.interpolations.is_empty())
                        && chain_calls(expr, None, None) != Some(false)
                });
            self.has_deferred_effects |= deferred_effects;
            self.bindings.push(LateBinding {
                name: &self.source[span.start..span.end],
                scope: self.scope,
                start: span.start,
                deferred_effects,
            });
        }
        falcon_syntax::visitor::walk_local_var(self, node);
    }

    fn visit_formal_param(&mut self, node: &FormalParam) {
        let span = node.name.span;
        self.bindings.push(LateBinding {
            name: &self.source[span.start..span.end],
            scope: self.scope,
            start: span.start,
            deferred_effects: false,
        });
        falcon_syntax::visitor::walk_formal_param(self, node);
    }
}
