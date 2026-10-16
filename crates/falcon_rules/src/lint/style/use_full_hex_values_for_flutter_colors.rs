//! Require an eight-digit hexadecimal integer for the canonical Flutter Color
//! default constructor.

use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, QualifiedName, Rule, RulePreparation, SemanticModel,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr};

#[derive(Default)]
pub struct UseFullHexValuesForFlutterColors;

impl Rule for UseFullHexValuesForFlutterColors {
    fn prepare<'a>(
        &self,
        program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let Some(identities) = ctx.identities else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        let model = SemanticModel::new(ctx.file_path, identities, ctx.types);
        let mut collector = Collector {
            model: &model,
            ctx,
            diags: Vec::new(),
        };
        collector.visit_program(program);
        RulePreparation::Diagnostics(collector.diags)
    }
}

struct Collector<'a> {
    model: &'a SemanticModel<'a>,
    ctx: &'a AnalyzeContext<'a>,
    diags: Vec<Diagnostic>,
}

impl Visitor for Collector<'_> {
    fn visit_expr(&mut self, node: &Expr) {
        if matches!(node, Expr::Paren { .. }) {
            falcon_syntax::visitor::walk_expr(self, node);
            return;
        }
        if let Some((identity, constructor, args)) = construction(node, self.model)
            && constructor == "new"
            && is_color(&identity)
            && let Some(Expr::IntLit { value, span }) =
                args.positional.first().map(Expr::unparenthesized)
            && !is_full_hex(value)
        {
            self.diags.push(self.ctx.diagnostic(
                "use-full-hex-values-for-flutter-colors",
                "Use the full 8-digit hexadecimal value (0xAARRGGBB) for a Color.",
                *span,
            ));
        }
        walk_expr(self, node);
    }
}

fn construction<'a>(
    expression: &'a Expr,
    model: &SemanticModel<'_>,
) -> Option<(DeclarationIdentity, &'a str, &'a ArgList)> {
    match expression.unparenthesized() {
        Expr::New {
            dart_type,
            constructor_name,
            args,
            ..
        } => {
            let DartType::Named(named) = dart_type else {
                return None;
            };
            let constructor = constructor_name
                .as_ref()
                .map_or("new", |name| name.name.as_str());
            Some((
                model.resolve_name(QualifiedName::from_identifiers(&named.segments)?)?,
                constructor,
                args,
            ))
        }
        Expr::Call { callee, args, .. } => {
            let (identity, constructor) =
                falcon_analyze::resolve_call_constructor(callee, model, false)?;
            Some((identity, constructor, args))
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
