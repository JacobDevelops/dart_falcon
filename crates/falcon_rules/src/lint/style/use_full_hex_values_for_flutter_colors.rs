//! Require an eight-digit hexadecimal integer for the canonical Flutter Color
//! default constructor.

use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, QualifiedName, Rule, RulePreparation, SemanticModel,
    StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct UseFullHexValuesForFlutterColors;

impl Rule for UseFullHexValuesForFlutterColors {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let Some(identities) = ctx.identities else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        let collector = Collector {
            model: SemanticModel::new(ctx.file_path, ctx.parsed, identities, ctx.types),
            ctx,
            diags: Vec::new(),
        };
        RulePreparation::Structural(Box::new(collector))
    }
}

struct Collector<'a> {
    model: SemanticModel<'a>,
    ctx: &'a AnalyzeContext<'a>,
    diags: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        if matches!(node, Expr::Paren(_)) {
            return StructuralTraversal::Continue;
        }
        if let Some((identity, constructor, args)) = construction(node, &self.model)
            && constructor == "new"
            && is_color(&identity)
            && let Some(Expr::IntLit(literal)) =
                args.positional().next().map(Expr::unparenthesized)
            && !is_full_hex(literal.value())
        {
            self.diags.push(self.ctx.diagnostic(
                "use-full-hex-values-for-flutter-colors",
                "Use the full 8-digit hexadecimal value (0xAARRGGBB) for a Color.",
                literal.span(),
            ));
        }
        StructuralTraversal::Continue
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }
}
}

fn construction<'a>(
    expression: Expr<'a>,
    model: &SemanticModel<'_>,
) -> Option<(DeclarationIdentity, &'a str, ArgList<'a>)> {
    match expression.unparenthesized() {
        Expr::New(new) => {
            let DartType::Named(named) = new.dart_type() else {
                return None;
            };
            let constructor = new.constructor_name().map_or("new", |name| name.name());
            Some((
                model.resolve_name(QualifiedName::from_identifiers(named.segments())?)?,
                constructor,
                new.args(),
            ))
        }
        Expr::Call(call) => {
            let (identity, constructor) =
                falcon_analyze::resolve_call_constructor(call.callee(), model, false)?;
            Some((identity, constructor, call.args()))
        }
        _ => None,
    }
}

fn is_color(identity: &DeclarationIdentity) -> bool {
    identity.is_sdk("dart:ui", "Color") || identity.is_package("flutter", "Color")
}

fn is_full_hex(value: &str) -> bool {
    let normalized = value.replace('_', "").to_ascii_lowercase();
    normalized.starts_with("0x") && normalized.len() == 10
}
