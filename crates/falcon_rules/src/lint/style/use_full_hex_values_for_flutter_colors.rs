//! Flags `Color(0x...)` with fewer than 8 hex digits. Ported from package:lints `use_full_hex_values_for_flutter_colors`.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr};

pub struct UseFullHexValuesForFlutterColors;

impl Rule for UseFullHexValuesForFlutterColors {
    fn name(&self) -> &'static str {
        "use-full-hex-values-for-flutter-colors"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut collector = Collector {
            diags: Vec::new(),
            file: ctx.file_path.to_string_lossy().into_owned(),
        };
        collector.visit_program(program);
        collector.diags
    }
}

struct Collector {
    diags: Vec<Diagnostic>,
    file: String,
}

impl Visitor for Collector {
    fn visit_expr(&mut self, node: &Expr) {
        if let Expr::Call {
            callee, args, span, ..
        } = node
            && let Expr::Ident(id) = callee.as_ref()
            && id.name == "Color"
            && args.named.is_empty()
            && args.positional.len() == 1
            && let Expr::IntLit { value, .. } = &args.positional[0]
            && is_short_hex(value)
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

/// True when `value` is a hex integer literal (`0x`/`0X`) whose hex-digit count
/// (ignoring `_` separators) is under 8, i.e. an alpha-less/short color value.
fn is_short_hex(value: &str) -> bool {
    let Some(digits) = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    else {
        return false;
    };
    let count = digits.chars().filter(|c| c.is_ascii_hexdigit()).count();
    count > 0 && count < 8
}
