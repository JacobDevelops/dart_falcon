//! Disallow statically known invalid `dart:core` regular expressions.
//!
//! Validate decoded literal patterns with an ECMAScript parser, including the
//! constructor's literal `unicode: true` flag, matching the SDK lint.

use crate::lint::string_lit::{has_interpolation, literal_parts, multiline_lead_len};
use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, ResolvedType, Rule, RulePreparation, SemanticModel,
    StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct ValidRegexps;

impl Rule for ValidRegexps {
    fn prepare<'a>(
        &self,
        _program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let Some(identities) = ctx.identities else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        let collector = Collector {
            diagnostics: Vec::new(),
            ctx,
            model: SemanticModel::new(ctx.file_path, identities, ctx.types),
        };
        RulePreparation::Structural(Box::new(collector))
    }
}

struct Collector<'a> {
    diagnostics: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
    model: SemanticModel<'a>,
}

impl Collector<'_> {
    fn check(&mut self, args: &ArgList) {
        let Some(argument) = args.positional.first() else {
            return;
        };
        if args
            .named
            .iter()
            .any(|arg| arg.span.start < argument.span().start)
        {
            return;
        }
        let unicode = args.named.iter().any(|arg| {
            arg.name.name == "unicode" && matches!(arg.value, Expr::BoolLit { value: true, .. })
        });
        if extractable_pattern(argument).is_some_and(|pattern| {
            let pattern = regexp_units(pattern, unicode);
            regress::Regex::from_unicode(
                pattern.iter().copied(),
                regress::Flags {
                    unicode,
                    no_opt: true,
                    ..Default::default()
                },
            )
            .is_err()
        }) {
            let span = argument.span();
            self.diagnostics.push(self.ctx.diagnostic(
                "valid-regexps",
                "Invalid regular expression.",
                *span,
            ));
        }
    }

    fn sdk_regexp_name(&self, expression: &Expr) -> bool {
        let segments = falcon_analyze::expression_segments(expression, false);
        matches!(segments.as_deref().and_then(|segments| self.model.resolve_dotted(segments)), Some(DeclarationIdentity::Sdk { library, name }) if library == "dart:core" && name == "RegExp")
    }
}

falcon_analyze::structural_visitor! {
impl StructuralRuleVisitor for Collector<'_> {
    fn visit_expr(&mut self, node: &Expr) -> StructuralTraversal {
        match node {
            Expr::Call { callee, args, .. } if self.sdk_regexp_name(callee) => self.check(args),
            Expr::New {
                dart_type, args, ..
            } if matches!(self.model.resolve_type(dart_type), ResolvedType::Interface { identity: DeclarationIdentity::Sdk { library, name }, .. } if library == "dart:core" && name == "RegExp") => {
                self.check(args)
            }
            _ => {}
        }
        StructuralTraversal::Continue
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }
}
}

fn extractable_pattern(expr: &Expr) -> Option<Vec<u32>> {
    let Expr::StringLit(node) = expr else {
        return None;
    };
    if !node.interpolations.is_empty() {
        return None;
    }
    let mut pattern = Vec::with_capacity(node.value.len());
    for (literal, _) in node.segments() {
        if !literal.terminated {
            return None;
        }
        let parts = literal_parts(&literal);
        if has_interpolation(&parts) {
            return None;
        }
        let trim = if literal.triple {
            multiline_lead_len(literal.content)
        } else {
            0
        };
        let mut chars = literal.content[trim..].chars();
        while let Some(c) = chars.next() {
            let value = if !literal.is_raw && c == '\\' {
                match chars.next()? {
                    'n' => 10,
                    'r' => 13,
                    't' => 9,
                    'b' => 8,
                    'f' => 12,
                    'v' => 11,
                    escape @ ('x' | 'u') => {
                        let braced = escape == 'u' && chars.clone().next() == Some('{');
                        if braced {
                            chars.next();
                        }
                        let count = if escape == 'x' { 2 } else { 4 };
                        let mut value = 0u32;
                        if braced {
                            loop {
                                let c = chars.next()?;
                                if c == '}' {
                                    break;
                                }
                                value = value.checked_mul(16)?.checked_add(c.to_digit(16)?)?;
                            }
                        } else {
                            for _ in 0..count {
                                value = value * 16 + chars.next()?.to_digit(16)?;
                            }
                        }
                        value
                    }
                    c => c as u32,
                }
            } else {
                c as u32
            };
            if value > 0x10ffff {
                return None;
            } else if value > 0xffff {
                pattern.push(0xd800 + ((value - 0x10000) >> 10));
                pattern.push(0xdc00 + ((value - 0x10000) & 0x3ff));
            } else {
                pattern.push(value);
            }
        }
    }
    Some(pattern)
}

fn starts_named_group(pattern: &[u32]) -> bool {
    pattern.starts_with(&[u32::from(b'('), u32::from(b'?'), u32::from(b'<')])
        && !matches!(pattern.get(3), Some(&0x3d | &0x21))
}

fn has_named_capture(pattern: &[u32]) -> bool {
    let mut in_class = false;
    let mut i = 0;
    while i < pattern.len() {
        match pattern[i] {
            0x5c => i += 1, // Skip escaped delimiters.
            0x5b => in_class = true,
            0x5d => in_class = false,
            _ if !in_class && starts_named_group(&pattern[i..]) => return true,
            _ => {}
        }
        i += 1;
    }
    false
}

/// Regress parses code points even without `u`; Dart parses UTF-16 code units.
/// Normalize the legacy escapes that regress otherwise interprets as Unicode.
fn regexp_units(mut pattern: Vec<u32>, unicode: bool) -> Vec<u32> {
    let mut read = 0;
    let mut write = 0;
    let mut in_class = false;
    let mut name_start = None;
    let has_named_capture = !unicode && has_named_capture(&pattern);
    while read < pattern.len() {
        let c = pattern[read];
        // Group identifiers use Unicode identifier syntax even in legacy mode.
        if !in_class && name_start.is_none() {
            let tail = &pattern[read..];
            if starts_named_group(tail)
                || (has_named_capture
                    && tail.starts_with(&[u32::from(b'\\'), u32::from(b'k'), u32::from(b'<')]))
            {
                name_start = Some(read + 3);
            }
        }
        let in_name = name_start.is_some_and(|start| read >= start);
        if in_name && c == u32::from(b'>') {
            name_start = None;
        }
        if c == u32::from(b'[') {
            in_class = true;
        } else if c == u32::from(b']') {
            in_class = false;
        }
        let mut value = c;
        let mut consumed = 1;
        if (unicode || in_name)
            && (0xd800..=0xdbff).contains(&c)
            && let Some(&low) = pattern.get(read + 1)
            && (0xdc00..=0xdfff).contains(&low)
        {
            value = 0x10000 + ((c - 0xd800) << 10) + low - 0xdc00;
            consumed = 2;
        } else if !unicode
            && !in_name
            && c == u32::from(b'\\')
            && pattern.get(read + 1) == Some(&u32::from(b'u'))
        {
            if pattern.get(read + 2) == Some(&u32::from(b'{')) {
                value = u32::from(b'u');
                consumed = 2;
            } else if let Some(surrogate) = pattern
                .get(read + 2..read + 6)
                .and_then(|digits| {
                    digits.iter().try_fold(0u32, |value, &digit| {
                        Some(value * 16 + char::from_u32(digit)?.to_digit(16)?)
                    })
                })
                .filter(|v| (0xd800..=0xdfff).contains(v))
            {
                value = surrogate;
                consumed = 6;
            }
        }
        pattern[write] = value;
        write += 1;
        read += consumed;
        // Preserve escaped backslashes as pairs instead of reinterpreting them.
        if !unicode && c == u32::from(b'\\') && consumed == 1 && read < pattern.len() {
            pattern[write] = pattern[read];
            write += 1;
            read += 1;
        }
    }
    pattern.truncate(write);
    pattern
}
