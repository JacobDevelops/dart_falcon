//! Flags unnamed numeric literals (magic numbers) used inline in expressions.
//!
//! A bare number buried in logic hides its meaning and invites duplication;
//! extracting it to a named constant documents intent and gives the value a
//! single point of change. To keep the rule practical it exempts contexts where
//! a literal is self-explanatory or unavoidable: variable, field, and top-level
//! initializers, direct elements of list/set literals, entries of const maps,
//! literal array indices, any const context (const constructors, const
//! collections), and `DateTime` constructor arguments. Detection also reaches
//! through cascades, records, switch expressions, and assert statements so
//! literals in those positions are still caught. Values on the allow-list are
//! never treated as magic.
//!
//! ## Options
//!
//! `allowed` (list of numbers, default: `[-1, 0, 1]`) — numeric values never
//! treated as magic numbers.

/// The `no-magic-number` rule.
pub use dcl::NoMagicNumber;

mod dcl {
    use std::collections::HashSet;

    use falcon_analyze::{
        AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
        structural_visitor,
    };
    use falcon_diagnostics::Diagnostic;
    use falcon_syntax::ast::*;

    #[derive(Default)]
    pub struct NoMagicNumber;

    const EPS: f64 = 1e-9;

    struct Cfg {
        allowed: Vec<f64>,
    }

    fn cfg(ctx: &AnalyzeContext) -> Cfg {
        let allowed = crate::lint::options::for_rule(ctx, "no-magic-number")
            .and_then(|o| o.get("allowed"))
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_f64()).collect::<Vec<_>>())
            .unwrap_or_else(|| vec![-1.0, 0.0, 1.0]);
        Cfg { allowed }
    }

    impl Rule for NoMagicNumber {
        fn prepare<'a>(
            &self,
            _program: &'a Program,
            ctx: &'a AnalyzeContext<'a>,
        ) -> RulePreparation<'a> {
            RulePreparation::Structural(Box::new(Collector {
                diags: Vec::new(),
                ctx,
                cfg: cfg(ctx),
                in_const: 0,
                exempt_literals: HashSet::new(),
            }))
        }
    }

    fn parse_num(value: &str) -> Option<f64> {
        let v = value.replace('_', "");
        if let Some(hex) = v.strip_prefix("0x").or_else(|| v.strip_prefix("0X")) {
            return i64::from_str_radix(hex, 16).ok().map(|n| n as f64);
        }
        if let Some(bin) = v.strip_prefix("0b").or_else(|| v.strip_prefix("0B")) {
            return i64::from_str_radix(bin, 2).ok().map(|n| n as f64);
        }
        v.parse::<f64>().ok()
    }

    fn is_allowed(value: &str, cfg: &Cfg) -> bool {
        match parse_num(value) {
            Some(n) => cfg.allowed.iter().any(|a| (a - n).abs() < EPS),
            None => true,
        }
    }

    fn root_name(expr: &Expr) -> Option<&str> {
        match expr.unparenthesized() {
            Expr::Ident(id) => Some(&id.name),
            Expr::Field { object, .. } => root_name(object),
            _ => None,
        }
    }

    fn type_base_name(dart_type: &DartType) -> Option<&str> {
        match dart_type {
            DartType::Named(n) => n.segments.last().map(|s| s.name.as_str()),
            _ => None,
        }
    }

    fn direct_literal_span(expr: &Expr) -> Option<usize> {
        let literal = match expr.unparenthesized() {
            Expr::Unary { operand, .. } => operand.as_ref(),
            _ => expr,
        };
        match literal.unparenthesized() {
            Expr::IntLit { span, .. } | Expr::DoubleLit { span, .. } => Some(span.start),
            _ => None,
        }
    }

    struct Collector<'a, 'ctx> {
        diags: Vec<Diagnostic>,
        ctx: &'a AnalyzeContext<'ctx>,
        cfg: Cfg,
        in_const: usize,
        exempt_literals: HashSet<usize>,
    }

    impl Collector<'_, '_> {
        fn flag(&mut self, value: &str, span: &Span) {
            if self.in_const != 0
                || self.exempt_literals.contains(&span.start)
                || is_allowed(value, &self.cfg)
            {
                return;
            }
            self.diags.push(self.ctx.diagnostic(
                "no-magic-number",
                "Avoid using magic numbers. Extract them to named constants or variables.",
                *span,
            ));
        }
    }

    structural_visitor! {
    impl StructuralRuleVisitor for Collector<'_, '_> {
        fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }

        fn visit_top_level_decl(&mut self, node: &TopLevelDecl) -> StructuralTraversal {
            if matches!(node, TopLevelDecl::Variable(_)) {
                StructuralTraversal::SkipChildren
            } else {
                StructuralTraversal::Continue
            }
        }

        fn visit_field_decl(&mut self, _node: &FieldDecl) -> StructuralTraversal {
            StructuralTraversal::SkipChildren
        }
        fn visit_formal_param(&mut self, _node: &FormalParam) -> StructuralTraversal {
            StructuralTraversal::SkipChildren
        }
        fn visit_local_var(&mut self, _node: &LocalVarDecl) -> StructuralTraversal {
            StructuralTraversal::SkipChildren
        }
        fn visit_stmt(&mut self, node: &Stmt) -> StructuralTraversal {
            if matches!(node, Stmt::PatternDecl(_)) {
                StructuralTraversal::SkipChildren
            } else {
                StructuralTraversal::Continue
            }
        }

        fn visit_expr(&mut self, node: &Expr) -> StructuralTraversal {
            if matches!(node, Expr::New { is_const: true, .. } | Expr::List { is_const: true, .. } | Expr::Set { is_const: true, .. } | Expr::Map { is_const: true, .. }) {
                self.in_const += 1;
            }
            match node {
                Expr::IntLit { value, span } | Expr::DoubleLit { value, span } => {
                    self.flag(value, span);
                }
                Expr::Call { callee, .. } if root_name(callee) == Some("DateTime") => {
                    return StructuralTraversal::SkipChildren;
                }
                Expr::New { dart_type, .. } if type_base_name(dart_type) == Some("DateTime") => {
                    return StructuralTraversal::SkipChildren;
                }
                Expr::List { elements, .. } | Expr::Set { elements, .. } => {
                    self.exempt_literals.extend(elements.iter().filter_map(|element| match element {
                        CollectionElement::Expr(expr) => direct_literal_span(expr),
                        _ => None,
                    }));
                }
                Expr::Index { index, .. } => {
                    self.exempt_literals.extend(direct_literal_span(index));
                }
                Expr::Cascade { sections, .. } => {
                    self.exempt_literals.extend(sections.iter().flat_map(|section| {
                        section.ops.iter().filter_map(|op| match op {
                            CascadeOp::Index { index, .. }
                            | CascadeOp::IndexAssign(index, _, _) => direct_literal_span(index),
                            _ => None,
                        })
                    }));
                }
                _ => {}
            }
            StructuralTraversal::Continue
        }

        fn exit_expr(&mut self, node: &Expr) {
            if matches!(node, Expr::New { is_const: true, .. } | Expr::List { is_const: true, .. } | Expr::Set { is_const: true, .. } | Expr::Map { is_const: true, .. }) {
                self.in_const -= 1;
            }
        }
    }
    }
}
