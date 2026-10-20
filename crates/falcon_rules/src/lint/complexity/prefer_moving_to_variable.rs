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

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, SemanticModel, StructuralRuleVisitor,
    StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::visit::Visitor;
use falcon_syntax::cst::visit::{body_list, member_list, stmt_lists};
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct PreferMovingToVariable;

impl Rule for PreferMovingToVariable {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            threshold: allowed_duplicated_chains(ctx),
            late: LateDeclarations {
                source: ctx.source(),
                bindings: Vec::new(),
                has_deferred_effects: false,
                scope: Span {
                    start: 0,
                    end: ctx.source().len(),
                },
            },
            model: ctx.identities().map(|identities| {
                SemanticModel::new(ctx.file_path(), ctx.parsed(), identities, ctx.types())
            }),
            lists: Vec::new(),
            scopes: Vec::new(),
            read_spans: Vec::new(),
            initializer: None,
            initializers: Vec::new(),
        }))
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

enum Entry {
    Reset,
    Initializer {
        span: Span,
        calls: Option<bool>,
        pure: bool,
    },
}

struct PendingList {
    span: Span,
    entries: Vec<Entry>,
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    threshold: usize,
    late: LateDeclarations<'a>,
    model: Option<SemanticModel<'a>>,
    lists: Vec<PendingList>,
    scopes: Vec<Span>,
    read_spans: Vec<(Span, Span)>,
    initializer: Option<Span>,
    initializers: Vec<Option<Span>>,
}

impl Collector<'_> {
    fn collect_list(&mut self, stmts: AstChildren<'_, Stmt<'_>>) {
        let (Some(first), Some(last)) = (stmts.first(), stmts.clone().last()) else {
            return;
        };
        let has_initializers = stmts.clone().any(|stmt| matches!(stmt, Stmt::LocalVar(local)
            if !local.is_late() && local.declarators().any(|declaration| declaration.initializer().is_some())));
        let mut entries = Vec::new();
        if has_initializers {
            for stmt in stmts {
                let Stmt::LocalVar(local) = stmt else {
                    entries.push(Entry::Reset);
                    continue;
                };
                if local.is_late() {
                    entries.push(Entry::Reset);
                    continue;
                }
                for declaration in local.declarators() {
                    let Some(init) = declaration.initializer() else {
                        continue;
                    };
                    let eligible = matches!(init.unparenthesized(), Expr::Field(_))
                        || matches!(init.unparenthesized(), Expr::Call(call)
                        if matches!(call.callee().unparenthesized(), Expr::Field(_)));
                    let calls = eligible
                        .then(|| chain_calls(init, self.model.as_ref(), self.ctx.signatures()))
                        .flatten();
                    let pure = matches!(
                        init.unparenthesized(),
                        Expr::IntLit(_)
                            | Expr::DoubleLit(_)
                            | Expr::BoolLit(_)
                            | Expr::NullLit(_)
                            | Expr::Ident(_)
                            | Expr::This(_)
                            | Expr::Super(_)
                            | Expr::FuncExpr(_)
                    ) || matches!(init.unparenthesized(), Expr::StringLit(literal) if literal.interpolations().next().is_none());
                    entries.push(Entry::Initializer {
                        span: init.span(),
                        calls,
                        pure,
                    });
                }
            }
        }
        self.lists.push(PendingList {
            span: Span {
                start: first.span().start,
                end: last.span().end,
            },
            entries,
        });
    }
}

fn chain_calls(
    expr: Expr<'_>,
    model: Option<&falcon_analyze::SemanticModel<'_>>,
    signatures: Option<&falcon_analyze::SignatureIndex>,
) -> Option<bool> {
    let mut probe = ChainProbe {
        model,
        signatures,
        next: Some(expr.span()),
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

impl<'a> Visitor<'a> for ChainProbe<'_, '_> {
    fn visit_expr(&mut self, node: Expr<'a>) {
        // Follow only the receiver/callee spine, never argument expressions.
        if self.next != Some(node.span()) || !self.valid {
            return;
        }
        self.next = match node {
            Expr::Field(field) => Some(field.object().span()),
            Expr::GenericInstantiation(instantiation) => Some(instantiation.target().span()),
            Expr::Call(call) => {
                let callee = call.callee();
                self.has_call = true;
                self.valid = matches!(callee.unparenthesized(), Expr::Field(field) if field.field().name() != "new")
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
                Some(callee.span())
            }
            Expr::Ident(_) | Expr::This(_) | Expr::Super(_) => None,
            _ => {
                self.valid = false;
                None
            }
        };
        falcon_syntax::cst::visit::walk_expr(self, node);
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

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(mut self: Box<Self>) -> Vec<Diagnostic> {
        if self.late.has_deferred_effects {
            self.read_spans.sort_unstable_by_key(|(initializer, _)| initializer.start);
            for list in &self.lists {
                for binding in &mut self.late.bindings {
                    if list.span.start <= binding.start && binding.start < list.span.end
                        && binding.scope.start <= list.span.start && list.span.end <= binding.scope.end
                    {
                        binding.scope = list.span;
                    }
                }
            }
        }
        let mut diagnostics = Vec::new();
        let mut counts = std::collections::HashMap::<&str, usize>::new();
        for list in &self.lists {
            counts.clear();
            for entry in &list.entries {
                let Entry::Initializer { span, calls, pure } = entry else {
                    counts.clear();
                    continue;
                };
                let deferred = self.late.has_deferred_effects && {
                    let start = self.read_spans.partition_point(|(initializer, _)| initializer.start < span.start);
                    let end = self.read_spans.partition_point(|(initializer, _)| initializer.start <= span.start);
                    self.read_spans[start..end].iter().any(|(_, read)| {
                    let name = &self.ctx.source()[read.start..read.end];
                    self.late.bindings.iter().filter(|binding| {
                        binding.name == name && binding.scope.start <= read.start
                            && read.end <= binding.scope.end && binding.start <= read.start
                    }).max_by_key(|binding| (binding.scope.start, binding.start))
                        .is_some_and(|binding| binding.deferred_effects)
                    })
                };
                if deferred {
                    counts.clear();
                    continue;
                }
                if let Some(has_call) = calls {
                    let source = &self.ctx.source()[span.start..span.end.min(self.ctx.source().len())];
                    if *has_call {
                        counts.retain(|previous, _| *previous == source);
                    }
                    let occurrence = counts.entry(source).and_modify(|count| *count += 1).or_insert(1);
                    if *occurrence >= self.threshold {
                        diagnostics.push(self.ctx.diagnostic(
                            "prefer-moving-to-variable",
                            "Duplicate expression — extract to a shared variable",
                            *span,
                        ));
                    }
                } else if !*pure {
                    counts.clear();
                }
            }
        }
        diagnostics
    }

    fn visit_function_decl(&mut self, node: FunctionDecl<'a>) -> StructuralTraversal {
        self.scopes.push(self.late.scope);
        self.late.scope = node.span();
        body_list(node.body(), &mut |stmts, _| self.collect_list(stmts));
        StructuralTraversal::Continue
    }

    fn exit_function_decl(&mut self, _node: FunctionDecl<'a>) {
        self.late.scope = self.scopes.pop().unwrap();
    }

    fn visit_class_member(&mut self, node: ClassMember<'a>) -> StructuralTraversal {
        self.scopes.push(self.late.scope);
        self.late.scope = node.span();
        member_list(node, &mut |stmts, _| self.collect_list(stmts));
        StructuralTraversal::Continue
    }

    fn exit_class_member(&mut self, _node: ClassMember<'a>) {
        self.late.scope = self.scopes.pop().unwrap();
    }

    fn visit_stmt(&mut self, node: Stmt<'a>) -> StructuralTraversal {
        self.scopes.push(self.late.scope);
        match node {
            Stmt::LocalFunc(function) => self.late.scope = function.span(),
            Stmt::Block(block) => self.late.scope = block.span(),
            _ => {}
        }
        stmt_lists(node, &mut |stmts, _| self.collect_list(stmts));
        StructuralTraversal::Continue
    }

    fn exit_stmt(&mut self, _node: Stmt<'a>) {
        self.late.scope = self.scopes.pop().unwrap();
    }

    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        self.scopes.push(self.late.scope);
        self.initializers.push(self.initializer);
        if let Expr::FuncExpr(function) = node {
            self.late.scope = node.span();
            self.initializer = None;
            body_list(Some(function.body()), &mut |stmts, _| self.collect_list(stmts));
        }
        StructuralTraversal::Continue
    }

    fn exit_expr(&mut self, _node: Expr<'a>) {
        self.initializer = self.initializers.pop().unwrap();
        self.late.scope = self.scopes.pop().unwrap();
    }

    fn visit_local_var(&mut self, node: LocalVarDecl<'a>) -> StructuralTraversal {
        for declaration in node.declarators() {
            let span = declaration.name().span();
            let deferred_effects = node.is_late() && declaration.initializer()
                .is_some_and(|expr| {
                    !matches!(expr.unparenthesized(), Expr::IntLit(_) | Expr::DoubleLit(_)
                        | Expr::BoolLit(_) | Expr::NullLit(_) | Expr::FuncExpr(_))
                        && !matches!(expr.unparenthesized(), Expr::StringLit(literal) if literal.interpolations().next().is_none())
                        && chain_calls(expr, None, None) != Some(false)
                });
            self.late.has_deferred_effects |= deferred_effects;
            self.late.bindings.push(LateBinding {
                name: &self.late.source[span.start..span.end],
                scope: self.late.scope, start: span.start, deferred_effects,
            });
        }
        StructuralTraversal::Continue
    }

    fn visit_formal_param(&mut self, node: FormalParam<'a>) -> StructuralTraversal {
        let span = node.name().span();
        self.late.bindings.push(LateBinding {
            name: &self.late.source[span.start..span.end],
            scope: self.late.scope, start: span.start, deferred_effects: false,
        });
        StructuralTraversal::Continue
    }

    fn visit_var_declarator(&mut self, node: VarDeclarator<'a>) -> StructuralTraversal {
        self.initializers.push(self.initializer);
        self.initializer = node.initializer().map(|expr| expr.span());
        StructuralTraversal::Continue
    }

    fn exit_var_declarator(&mut self, _node: VarDeclarator<'a>) {
        self.initializer = self.initializers.pop().unwrap();
    }

    fn visit_identifier(&mut self, node: Identifier<'a>) -> StructuralTraversal {
        if self.initializer.is_some_and(|span| span.start <= node.span().start && node.span().end <= span.end) {
            self.read_spans.push((self.initializer.unwrap(), node.span()));
        }
        StructuralTraversal::Continue
    }
}
}
