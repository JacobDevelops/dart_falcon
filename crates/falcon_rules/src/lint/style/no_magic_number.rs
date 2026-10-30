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

#[derive(serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) struct Options {
    /// Numeric values that are exempt from magic-number diagnostics.
    allowed: Vec<f64>,
}

impl Default for Options {
    fn default() -> Self {
        Self { allowed: vec![-1.0, 0.0, 1.0] }
    }
}

crate::lint::options::impl_options!(Options);

mod dcl {
    use std::collections::HashSet;

    use falcon_analyze::{
        AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
        structural_visitor,
    };
    use falcon_diagnostics::Diagnostic;
    use falcon_syntax::cst::*;
    use super::Options;

    #[derive(Default)]
    pub struct NoMagicNumber;

    const EPS: f64 = 1e-9;


    impl Rule for NoMagicNumber {
        fn prepare<'a>(
            &self,
            _program: Program<'a>,
            ctx: &'a AnalyzeContext<'a>,
        ) -> RulePreparation<'a> {
            RulePreparation::Structural(Box::new(Collector {
                diags: Vec::new(),
                ctx,
                cfg: crate::lint::options::get::<Options>(ctx, Self::NAME),
                in_const: 0,
                exempt_literals: HashSet::new(),
            }))
        }
    }

    const U32_BASE: f64 = 4_294_967_296.0;

    // Integer spellings are normalized to f64 for comparison with decimal literals.
    // Split into exact u32 chunks so the only rounding is the required f64 arithmetic.
    fn integer_as_f64(number: i64) -> f64 {
        let magnitude = number.unsigned_abs();
        let high = u32::try_from(magnitude >> 32).expect("shifted u64 fits u32");
        let low = u32::try_from(magnitude & u64::from(u32::MAX)).expect("masked u64 fits u32");
        let value = f64::from(high) * U32_BASE + f64::from(low);
        if number < 0 { -value } else { value }
    }

    fn parse_num(value: &str) -> Option<f64> {
        let v = value.replace('_', "");
        if let Some(hex) = v.strip_prefix("0x").or_else(|| v.strip_prefix("0X")) {
            return i64::from_str_radix(hex, 16).ok().map(integer_as_f64);
        }
        if let Some(bin) = v.strip_prefix("0b").or_else(|| v.strip_prefix("0B")) {
            return i64::from_str_radix(bin, 2).ok().map(integer_as_f64);
        }
        v.parse::<f64>().ok()
    }

    fn is_allowed(value: &str, cfg: &Options) -> bool {
        match parse_num(value) {
            Some(n) => cfg.allowed.iter().any(|a| (a - n).abs() < EPS),
            None => true,
        }
    }

    fn root_name(expr: Expr<'_>) -> Option<&str> {
        match expr.unparenthesized() {
            Expr::Ident(id) => Some(id.name()),
            Expr::Field(field) => root_name(field.object()),
            _ => None,
        }
    }

    fn type_base_name(dart_type: DartType<'_>) -> Option<&str> {
        match dart_type {
            DartType::Named(n) => n.segments().last().map(Identifier::name),
            _ => None,
        }
    }

    fn direct_literal_span(expr: Expr<'_>) -> Option<usize> {
        let literal = match expr.unparenthesized() {
            Expr::Unary(unary) => unary.operand(),
            _ => expr,
        };
        match literal.unparenthesized() {
            Expr::IntLit(literal) => Some(literal.span().start),
            Expr::DoubleLit(literal) => Some(literal.span().start),
            _ => None,
        }
    }

    fn explicit_const_context(expr: Expr<'_>) -> bool {
        match expr {
            Expr::New(expr) => expr.is_const(),
            Expr::List(expr) => expr.is_const(),
            Expr::Set(expr) => expr.is_const(),
            Expr::Map(expr) => expr.is_const(),
            _ => false,
        }
    }

    struct Collector<'a, 'ctx> {
        diags: Vec<Diagnostic>,
        ctx: &'a AnalyzeContext<'ctx>,
        cfg: &'a Options,
        in_const: usize,
        exempt_literals: HashSet<usize>,
    }

    impl Collector<'_, '_> {
        fn exempt_elements(&mut self, elements: AstChildren<'_, CollectionElement<'_>>) {
            self.exempt_literals
                .extend(elements.filter_map(|element| match element {
                    CollectionElement::Expr(expr) => direct_literal_span(expr),
                    _ => None,
                }));
        }

        fn flag(&mut self, value: &str, span: &Span) {
            if self.in_const != 0
                || self.exempt_literals.contains(&span.start)
                || is_allowed(value, self.cfg)
            {
                return;
            }
            self.diags.push(self.ctx.diagnostic("Avoid using magic numbers. Extract them to named constants or variables.", *span));
        }
    }

    structural_visitor! {
    impl<'a, 'ctx> StructuralRuleVisitor<'ctx> for Collector<'a, 'ctx> {
        fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diags }

        fn visit_top_level_decl(&mut self, node: TopLevelDecl<'ctx>) -> StructuralTraversal {
            if matches!(node, TopLevelDecl::Variable(_)) {
                StructuralTraversal::SkipChildren
            } else {
                StructuralTraversal::Continue
            }
        }

        fn visit_field_decl(&mut self, _node: FieldDecl<'ctx>) -> StructuralTraversal {
            StructuralTraversal::SkipChildren
        }
        fn visit_formal_param(&mut self, _node: FormalParam<'ctx>) -> StructuralTraversal {
            StructuralTraversal::SkipChildren
        }
        fn visit_local_var(&mut self, _node: LocalVarDecl<'ctx>) -> StructuralTraversal {
            StructuralTraversal::SkipChildren
        }
        fn visit_stmt(&mut self, node: Stmt<'ctx>) -> StructuralTraversal {
            if matches!(node, Stmt::PatternDecl(_)) {
                StructuralTraversal::SkipChildren
            } else {
                StructuralTraversal::Continue
            }
        }

        fn visit_expr(&mut self, node: Expr<'ctx>) -> StructuralTraversal {
            if explicit_const_context(node) {
                self.in_const += 1;
            }
            match node {
                Expr::IntLit(literal) => self.flag(literal.value(), &literal.span()),
                Expr::DoubleLit(literal) => self.flag(literal.value(), &literal.span()),
                Expr::Call(call) if root_name(call.callee()) == Some("DateTime") => {
                    return StructuralTraversal::SkipChildren;
                }
                Expr::New(expr) if type_base_name(expr.dart_type()) == Some("DateTime") => {
                    return StructuralTraversal::SkipChildren;
                }
                Expr::List(list) => self.exempt_elements(list.elements()),
                Expr::Set(set) => self.exempt_elements(set.elements()),
                Expr::Index(expr) => {
                    self.exempt_literals.extend(direct_literal_span(expr.index()));
                }
                Expr::Cascade(cascade) => {
                    self.exempt_literals.extend(cascade.sections().flat_map(|section| {
                        section.ops().filter_map(|op| match op {
                            CascadeOp::Index(index) => direct_literal_span(index.index()),
                            CascadeOp::IndexAssign(assign) => direct_literal_span(assign.target()),
                            _ => None,
                        })
                    }));
                }
                _ => {}
            }
            StructuralTraversal::Continue
        }

        fn exit_expr(&mut self, node: Expr<'ctx>) {
            if explicit_const_context(node) {
                self.in_const -= 1;
            }
        }
    }
    }
}
