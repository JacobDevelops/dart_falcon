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

    use falcon_analyze::{AnalyzeContext, Rule};
    use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
    use falcon_syntax::ast::*;
    use falcon_syntax::visitor::{Visitor, walk_expr, walk_stmt, walk_top_level_decl};

    pub struct NoMagicNumber;

    const EPS: f64 = 1e-9;

    struct Cfg {
        allowed: Vec<f64>,
    }

    fn cfg(ctx: &AnalyzeContext) -> Cfg {
        let allowed = crate::meta::meta_for("no-magic-number")
            .and_then(|m| ctx.rule_options(m.group, "no-magic-number"))
            .and_then(|o| o.get("allowed"))
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_f64()).collect::<Vec<_>>())
            .unwrap_or_else(|| vec![-1.0, 0.0, 1.0]);
        Cfg { allowed }
    }

    impl Rule for NoMagicNumber {
        fn name(&self) -> &'static str {
            "no-magic-number"
        }

        fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
            let cfg = cfg(ctx);
            let mut collector = Collector {
                diags: Vec::new(),
                ctx,
                cfg: &cfg,
                in_const: false,
                exempt_literals: HashSet::new(),
            };
            collector.visit_program(program);
            collector.diags
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
        match expr {
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
        let literal = match expr {
            Expr::Unary { operand, .. } => operand.as_ref(),
            _ => expr,
        };
        match literal {
            Expr::IntLit { span, .. } | Expr::DoubleLit { span, .. } => Some(span.start),
            _ => None,
        }
    }

    struct Collector<'a, 'ctx, 'cfg> {
        diags: Vec<Diagnostic>,
        ctx: &'a AnalyzeContext<'ctx>,
        cfg: &'cfg Cfg,
        in_const: bool,
        exempt_literals: HashSet<usize>,
    }

    impl Collector<'_, '_, '_> {
        fn with_const(&mut self, is_const: bool, walk: impl FnOnce(&mut Self)) {
            let previous = self.in_const;
            self.in_const |= is_const;
            walk(self);
            self.in_const = previous;
        }

        fn with_exempt_literals(
            &mut self,
            spans: impl IntoIterator<Item = usize>,
            walk: impl FnOnce(&mut Self),
        ) {
            let previous = self.exempt_literals.clone();
            self.exempt_literals.extend(spans);
            walk(self);
            self.exempt_literals = previous;
        }

        fn flag(&mut self, value: &str, span: &Span) {
            if self.in_const
                || self.exempt_literals.contains(&span.start)
                || is_allowed(value, self.cfg)
            {
                return;
            }
            self.diags.push(Diagnostic::new(
                "no-magic-number",
                Severity::Warning,
                "Avoid using magic numbers. Extract them to named constants or variables.",
                self.ctx.file_path.to_string_lossy().into_owned(),
                DiagSpan {
                    start: span.start,
                    end: span.end,
                },
            ));
        }
    }

    impl Visitor for Collector<'_, '_, '_> {
        fn visit_top_level_decl(&mut self, node: &TopLevelDecl) {
            if !matches!(node, TopLevelDecl::Variable(_)) {
                walk_top_level_decl(self, node);
            }
        }

        fn visit_field_decl(&mut self, _node: &FieldDecl) {}

        fn visit_formal_param(&mut self, _node: &FormalParam) {}

        fn visit_stmt(&mut self, node: &Stmt) {
            if !matches!(node, Stmt::LocalVar(_) | Stmt::PatternDecl(_)) {
                walk_stmt(self, node);
            }
        }

        fn visit_expr(&mut self, node: &Expr) {
            match node {
                Expr::IntLit { value, span } | Expr::DoubleLit { value, span } => {
                    self.flag(value, span);
                }
                Expr::Call { callee, .. } if root_name(callee) == Some("DateTime") => {}
                Expr::New {
                    is_const,
                    dart_type,
                    ..
                } if type_base_name(dart_type) != Some("DateTime") => {
                    self.with_const(*is_const, |this| walk_expr(this, node));
                }
                Expr::New { .. } => {}
                Expr::List {
                    is_const, elements, ..
                }
                | Expr::Set {
                    is_const, elements, ..
                } => {
                    let direct = elements.iter().filter_map(|element| match element {
                        CollectionElement::Expr(expr) => direct_literal_span(expr),
                        _ => None,
                    });
                    let spans: Vec<_> = direct.collect();
                    self.with_exempt_literals(spans, |this| {
                        this.with_const(*is_const, |this| walk_expr(this, node));
                    });
                }
                Expr::Map { is_const, .. } => {
                    self.with_const(*is_const, |this| walk_expr(this, node));
                }
                Expr::Index { index, .. } => {
                    self.with_exempt_literals(direct_literal_span(index), |this| {
                        walk_expr(this, node)
                    });
                }
                Expr::Cascade { sections, .. } => {
                    let spans = sections.iter().flat_map(|section| {
                        section.ops.iter().filter_map(|op| match op {
                            CascadeOp::Index(index, _) => direct_literal_span(index),
                            _ => None,
                        })
                    });
                    let spans: Vec<_> = spans.collect();
                    self.with_exempt_literals(spans, |this| walk_expr(this, node));
                }
                _ => walk_expr(self, node),
            }
        }
    }
}
