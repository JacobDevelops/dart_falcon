//! Require an eight-digit hexadecimal integer for the canonical Flutter Color
//! default constructor.

use falcon_analyze::{AnalyzeContext, DeclarationIdentity, Rule, SemanticModel};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr};

pub struct UseFullHexValuesForFlutterColors;

impl Rule for UseFullHexValuesForFlutterColors {
    fn name(&self) -> &'static str {
        "use-full-hex-values-for-flutter-colors"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(identities) = ctx.identities else {
            return Vec::new();
        };
        let model = SemanticModel::new(ctx.file_path, identities, ctx.types);
        let mut collector = Collector {
            model: &model,
            file: ctx.file_path.to_string_lossy().into_owned(),
            diags: Vec::new(),
        };
        collector.visit_program(program);
        collector.diags
    }
}

struct Collector<'a> {
    model: &'a SemanticModel<'a>,
    file: String,
    diags: Vec<Diagnostic>,
}

impl Visitor for Collector<'_> {
    fn visit_expr(&mut self, node: &Expr) {
        if let Some((identity, constructor, args)) = construction(node, self.model)
            && constructor == "new"
            && is_color(&identity)
            && let Some(Expr::IntLit { value, span }) = args.positional.first()
            && !is_full_hex(value)
        {
            self.diags.push(Diagnostic::new(
                "use-full-hex-values-for-flutter-colors",
                Severity::Warning,
                "Use the full 8-digit hexadecimal value (0xAARRGGBB) for a Color.",
                self.file.clone(),
                DiagSpan {
                    start: span.start,
                    end: span.end,
                },
            ));
        }
        walk_expr(self, node);
    }
}

fn construction<'a>(
    expression: &'a Expr,
    model: &SemanticModel<'_>,
) -> Option<(DeclarationIdentity, String, &'a ArgList)> {
    match expression {
        Expr::New {
            dart_type, args, ..
        } => {
            let DartType::Named(named) = dart_type else {
                return None;
            };
            let segments = named
                .segments
                .iter()
                .map(|segment| segment.name.clone())
                .collect::<Vec<_>>();
            Some((model.resolve_name(&segments)?, "new".to_string(), args))
        }
        Expr::Call { callee, args, .. } => {
            let mut segments = expression_segments(callee)?;
            if let Some(identity) = model.resolve_name(&segments) {
                return Some((identity, "new".to_string(), args));
            }
            let constructor = segments.pop()?;
            Some((model.resolve_name(&segments)?, constructor, args))
        }
        _ => None,
    }
}

fn expression_segments(expression: &Expr) -> Option<Vec<String>> {
    let mut current = expression;
    let mut segments = Vec::new();
    loop {
        match current {
            Expr::Ident(identifier) => {
                segments.push(identifier.name.clone());
                segments.reverse();
                return Some(segments);
            }
            Expr::Field { object, field, .. } => {
                segments.push(field.name.clone());
                current = object;
            }
            _ => return None,
        }
    }
}

fn is_color(identity: &DeclarationIdentity) -> bool {
    matches!(identity,
        DeclarationIdentity::Sdk { library, name }
            if library == "dart:ui" && name == "Color")
        || matches!(identity,
            DeclarationIdentity::Package { package, name }
                if package == "flutter" && name == "Color")
}

fn is_full_hex(value: &str) -> bool {
    let normalized = value.replace('_', "").to_ascii_lowercase();
    normalized.starts_with("0x") && normalized.len() == 10
}
