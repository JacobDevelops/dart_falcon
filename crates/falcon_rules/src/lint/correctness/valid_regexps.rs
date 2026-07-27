//! Disallow statically known invalid `dart:core` regular expressions.

use falcon_analyze::{AnalyzeContext, DeclarationIdentity, ResolvedType, Rule, SemanticModel};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr};

pub struct ValidRegexps;

impl Rule for ValidRegexps {
    fn name(&self) -> &'static str {
        "valid-regexps"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(identities) = ctx.identities else {
            return Vec::new();
        };
        let mut collector = Collector {
            diagnostics: Vec::new(),
            file: ctx.file_path.to_string_lossy().into_owned(),
            model: SemanticModel::new(ctx.file_path, identities, ctx.types),
        };
        collector.visit_program(program);
        collector.diagnostics
    }
}

struct Collector<'a> {
    diagnostics: Vec<Diagnostic>,
    file: String,
    model: SemanticModel<'a>,
}

impl Collector<'_> {
    fn check(&mut self, args: &ArgList) {
        let Some(argument) = args.positional.first() else {
            return;
        };
        if extractable_pattern(argument).is_some_and(|pattern| !valid_pattern(&pattern)) {
            let span = argument.span();
            self.diagnostics.push(Diagnostic::new(
                "valid-regexps",
                Severity::Warning,
                "Invalid regular expression.",
                self.file.clone(),
                DiagSpan {
                    start: span.start,
                    end: span.end,
                },
            ));
        }
    }

    fn sdk_regexp_name(&self, expression: &Expr) -> bool {
        let segments = expression_segments(expression);
        matches!(segments.as_deref().and_then(|segments| self.model.resolve_name(segments)), Some(DeclarationIdentity::Sdk { library, name }) if library == "dart:core" && name == "RegExp")
    }
}

impl Visitor for Collector<'_> {
    fn visit_expr(&mut self, node: &Expr) {
        match node {
            Expr::Call { callee, args, .. } if self.sdk_regexp_name(callee) => self.check(args),
            Expr::New {
                dart_type, args, ..
            } if matches!(self.model.resolve_type(dart_type), ResolvedType::Interface { identity: DeclarationIdentity::Sdk { library, name }, .. } if library == "dart:core" && name == "RegExp") => {
                self.check(args)
            }
            _ => {}
        }
        walk_expr(self, node);
    }
}

fn expression_segments(expression: &Expr) -> Option<Vec<String>> {
    match expression {
        Expr::Ident(identifier) => Some(vec![identifier.name.clone()]),
        Expr::Field { object, field, .. } => {
            let mut segments = expression_segments(object)?;
            segments.push(field.name.clone());
            Some(segments)
        }
        _ => None,
    }
}

fn extractable_pattern(expr: &Expr) -> Option<String> {
    let Expr::StringLit(node) = expr else {
        return None;
    };
    if !node.interpolations.is_empty() {
        return None;
    }
    let raw = &node.raw;
    let is_raw = raw.starts_with('r');
    let prefix = usize::from(is_raw);
    let rest = &raw[prefix..];
    let quote = if rest.starts_with("'''") {
        "'''"
    } else if rest.starts_with("\"\"\"") {
        "\"\"\""
    } else if rest.starts_with('\'') {
        "'"
    } else if rest.starts_with('"') {
        "\""
    } else {
        return None;
    };
    // Unterminated literals (parser error recovery) have no closing delimiter.
    let content = rest.strip_prefix(quote)?.strip_suffix(quote)?;
    if !is_raw && content.contains('\\') {
        return None;
    }
    Some(content.to_string())
}

fn valid_pattern(pattern: &str) -> bool {
    let chars: Vec<char> = pattern.chars().collect();
    let mut groups = 0usize;
    let mut in_class = false;
    let mut class_start = 0usize;
    let mut escaped = false;
    let mut can_quantify = false;
    let mut index = 0usize;
    while index < chars.len() {
        let ch = chars[index];
        if escaped {
            if ch == 'x' && !has_hex(&chars, index + 1, 2) {
                return false;
            }
            if ch == 'u' && !has_hex(&chars, index + 1, 4) {
                return false;
            }
            index += if ch == 'x' {
                2
            } else if ch == 'u' {
                4
            } else {
                0
            };
            escaped = false;
            can_quantify = true;
        } else if ch == '\\' {
            escaped = true;
        } else if in_class {
            // ECMAScript: `]` always closes, so `[]` is an empty class, and a
            // hyphen before `]` is a literal rather than a range.
            if ch == ']' {
                in_class = false;
                can_quantify = true;
            } else if ch == '-'
                && index > class_start
                && index + 1 < chars.len()
                && chars[index + 1] != ']'
                && chars[index - 1] > chars[index + 1]
            {
                return false;
            }
        } else {
            match ch {
                '[' => {
                    in_class = true;
                    class_start = index + 1;
                    can_quantify = false;
                }
                '(' => {
                    groups += 1;
                    can_quantify = false;
                }
                ')' => {
                    if groups == 0 {
                        return false;
                    }
                    groups -= 1;
                    can_quantify = true;
                }
                '|' => can_quantify = false,
                '*' | '+' => {
                    if !can_quantify {
                        return false;
                    }
                    can_quantify = false;
                }
                '?' if index > 0 && chars[index - 1] == '(' => can_quantify = false,
                '?' if can_quantify => can_quantify = false,
                '?' if index > 0 && matches!(chars[index - 1], '*' | '+' | '?' | '}') => {
                    can_quantify = false;
                }
                '?' => return false,
                '{' => {
                    if !can_quantify {
                        return false;
                    }
                    let Some(close) = chars[index + 1..]
                        .iter()
                        .position(|candidate| *candidate == '}')
                        .map(|offset| index + 1 + offset)
                    else {
                        return false;
                    };
                    let parts: Vec<_> = chars[index + 1..close]
                        .split(|candidate| *candidate == ',')
                        .collect();
                    if parts.is_empty()
                        || parts.len() > 2
                        || parts[0].is_empty()
                        || parts
                            .iter()
                            .flat_map(|part| part.iter())
                            .any(|candidate| !candidate.is_ascii_digit())
                    {
                        return false;
                    }
                    let minimum: usize = parts[0].iter().collect::<String>().parse().unwrap_or(0);
                    if parts.len() == 2 && !parts[1].is_empty() {
                        let maximum: usize =
                            parts[1].iter().collect::<String>().parse().unwrap_or(0);
                        if maximum < minimum {
                            return false;
                        }
                    }
                    index = close;
                    can_quantify = false;
                }
                _ => can_quantify = true,
            }
        }
        index += 1;
    }
    !escaped && !in_class && groups == 0
}

fn has_hex(chars: &[char], start: usize, count: usize) -> bool {
    chars
        .get(start..start + count)
        .is_some_and(|digits| digits.iter().all(|digit| digit.is_ascii_hexdigit()))
}
