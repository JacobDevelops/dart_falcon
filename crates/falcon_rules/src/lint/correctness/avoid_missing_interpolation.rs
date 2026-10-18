//! Flags missing interpolation around visible String variables or member accesses and calls.
//!
//! A plain string equal to an in-scope variable name may print the name instead
//! of its value. This check requires a positional argument of an unqualified call
//! and a local binding with a declared nonnullable `dart:core` String type.
//! Constructors, receiver methods, annotations, named arguments, raw/escaped/adjacent
//! strings, and calls whose arguments already read that variable are excluded.
//! Keys in maps, indexes, comparisons, and patterns are not positional arguments.
//!
//! In `'$object.property'` and `'$function(value)'`, only the identifier is
//! interpolated. The rule requires the receiver to have the named readable
//! member, or a known callable type, before suggesting `${...}`. Unknown members,
//! dynamic receivers, filenames such as `'assets/$name.png'`, and string-valued
//! method names in logs are left alone. Raw and escaped dollars and braced
//! interpolations are never flagged. Reports the first incomplete interpolation
//! in each segment, rather than an entire multiline literal.

use crate::lint::string_lit::{interpolated_literals, is_ident_body, is_simple_identifier};
use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, MemberKind, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::TokenKind;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidMissingInterpolation;

impl Rule for AvoidMissingInterpolation {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Collector {
            diags: Vec::new(),
            ctx,
        }))
    }
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

impl<'a> Collector<'a> {
    fn check_plain_arguments(&mut self, call: CallExpr<'a>, state: &SemanticState<'a>) {
        if !matches!(call.callee().unparenthesized(), Expr::Ident(_))
            || state.is_constructor_invocation(call.callee())
            || call
                .syntax()
                .ancestors()
                .any(|node| Annotation::cast(node).is_some())
        {
            return;
        }
        for argument in call.args().positional() {
            let Expr::StringLit(string) = argument.unparenthesized() else {
                continue;
            };
            let Some(literal) = string.delimiter() else {
                continue;
            };
            let name = literal.content;
            if literal.is_raw
                || !literal.terminated
                || !is_simple_identifier(name)
                || state.environment.binding_declaration(name).is_none()
                || !state.is_variable_read(name)
            {
                continue;
            }
            let ty = state.environment.declared_type(name);
            if !ty.interface("dart:core", "String") || ty.nullable() {
                continue;
            }
            let already_used = call.args().syntax().descendants().any(|node| {
                matches!(Expr::cast(node), Some(Expr::Ident(identifier)) if identifier.name() == name)
            });
            if already_used {
                continue;
            }
            self.diags.push(self.ctx.diagnostic(
                "avoid-missing-interpolation",
                "This string is a visible String variable's name; use the variable or interpolate its value.",
                string.span(),
            ));
        }
    }
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        if let Expr::Call(call) = node {
            self.check_plain_arguments(call, state);
        }
        let Expr::StringLit(string) = node else {
            return;
        };
        for (literal, span) in interpolated_literals(string) {
            let content_start = span.start + literal.content_offset;
            let incomplete = string.interpolations().find_map(|interpolation| {
                if !interpolation
                    .syntax()
                    .first_token()
                    .is_some_and(|token| token.kind() == TokenKind::InterpolationDollar)
                {
                    return None;
                }
                let expression_span = interpolation.span();
                let end = expression_span.end;
                if end < content_start || end > content_start + literal.content.len() {
                    return None;
                }
                let suffix = &literal.content[end - content_start..];
                let ty = state.infer(interpolation.expr());
                if ty.nullable() {
                    return None;
                }
                let suffix_len = if suffix.starts_with('(') {
                    let callable = ty.function_return_type().is_some()
                        || state
                            .signatures
                            .resolved_member_facts(&ty, "call", &state.model)
                            .is_some_and(|facts| {
                                facts
                                    .iter()
                                    .any(|fact| !fact.is_static && fact.kind == MemberKind::Method)
                            });
                    if !callable {
                        return None;
                    }
                    suffix.find(')')? + 1
                } else {
                    let member = suffix.strip_prefix('.')?;
                    let member_len = member
                        .bytes()
                        .take_while(|byte| is_ident_body(*byte))
                        .count();
                    let member = &member[..member_len];
                    if !is_simple_identifier(member) {
                        return None;
                    }
                    let readable = ty.record_field(member).is_some()
                        || state
                            .signatures
                            .resolved_member_facts(&ty, member, &state.model)
                            .is_some_and(|facts| {
                                facts
                                    .iter()
                                    .any(|fact| !fact.is_static && fact.kind != MemberKind::Setter)
                            });
                    if !readable {
                        return None;
                    }
                    member_len + 1
                };
                Some(Span::new(expression_span.start - 1, end + suffix_len))
            });
            if let Some(span) = incomplete {
                self.diags.push(self.ctx.diagnostic(
                    "avoid-missing-interpolation",
                    "Interpolation leaves a member access or call as literal text; wrap the complete expression in ${...}.",
                    span,
                ));
            }
        }
    }
}
