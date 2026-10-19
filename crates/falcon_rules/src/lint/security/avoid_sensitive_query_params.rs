//! Detect explicit secret-bearing URL query keys, not arbitrary occurrences of
//! words such as `token` in paths, fragments, map values, or prose.
use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidSensitiveQueryParams;

impl Rule for AvoidSensitiveQueryParams {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Collector {
            ctx,
            diags: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diags: Vec<Diagnostic>,
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        match node {
            Expr::StringLit(literal) => {
                if has_sensitive_query(literal, false) {
                    self.report(node.span());
                }
            }
            Expr::Call(call) => {
                let callee = call.callee().unparenthesized();
                let Some(segments) = falcon_analyze::expression_segments(callee, true) else {
                    return;
                };
                if state.environment.is_bound(segments[0]) {
                    return;
                }
                if let Some((identity, constructor)) =
                    falcon_analyze::resolve_call_constructor(callee, &state.model, true)
                    && state.model.unaliased(identity).is_sdk("dart:core", "Uri")
                    && matches!(constructor, "new" | "http" | "https")
                {
                    self.check_arguments(call.args(), matches!(constructor, "http" | "https"));
                }
            }
            Expr::New(new) => {
                if let Some((identity, constructor)) = falcon_analyze::resolve_new_constructor(
                    new.dart_type(),
                    new.constructor_name(),
                    &state.model,
                ) && state.model.unaliased(identity).is_sdk("dart:core", "Uri")
                    && matches!(constructor, "new" | "http" | "https")
                {
                    self.check_arguments(new.args(), matches!(constructor, "http" | "https"));
                }
            }
            _ => {}
        }
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }
}

impl<'a> Collector<'a> {
    fn check_arguments(&mut self, args: ArgList<'a>, positional_query: bool) {
        for arg in args.named() {
            if arg.name().name() == "queryParameters" {
                self.check_map(arg.value());
            } else if arg.name().name() == "query"
                && let Expr::StringLit(query) = arg.value().unparenthesized()
                && has_sensitive_query(query, true)
            {
                self.report(arg.value().span());
            }
        }
        if positional_query && let Some(query) = args.positional().nth(2) {
            self.check_map(query);
        }
    }

    fn check_map(&mut self, expr: Expr<'a>) {
        if let Expr::Map(map) = expr.unparenthesized() {
            for element in map.elements() {
                if let MapElement::Entry(entry) = element
                    && let Expr::StringLit(key) = entry.key().unparenthesized()
                    && key.interpolations().next().is_none()
                {
                    let mut segments = key.segments();
                    if let Some((segment, _)) = segments.next()
                        && segments.next().is_none()
                        && sensitive(segment.content)
                    {
                        self.report(entry.key().span());
                    }
                }
            }
        }
    }

    fn report(&mut self, span: Span) {
        self.diags.push(self.ctx.diagnostic(
            "avoid-sensitive-query-params",
            "Do not put secrets in URL query parameters; use an authorization header or request body.",
            span,
        ));
    }
}

fn has_sensitive_query(literal: StringLitNode<'_>, query_only: bool) -> bool {
    let mut at_key_start = query_only;
    for (segment, span) in literal.segments() {
        let text = segment.content;
        if !query_only && !text.starts_with("https://") && !text.starts_with("http://") {
            continue;
        }
        let offset = span.start + segment.content_offset;
        let mut interpolations = literal
            .interpolations()
            .map(|node| node.syntax().span())
            .filter(|interpolation| interpolation.start >= offset && interpolation.end <= span.end)
            .peekable();
        let mut query = query_only;
        let mut key_start = (query_only && at_key_start).then_some(0);
        let mut index = 0;
        while index < text.len() {
            if let Some(interpolation) = interpolations.peek()
                && interpolation.start == offset + index
            {
                // An interpolation's source punctuation is not URL text, and a
                // key containing a dynamic part cannot be identified statically.
                index = interpolation.end - offset;
                key_start = None;
                interpolations.next();
                continue;
            }
            match text.as_bytes()[index] {
                b'#' if !query_only => break,
                b'?' if !query => {
                    query = true;
                    key_start = Some(index + 1);
                }
                b'&' if query => key_start = Some(index + 1),
                b'=' if query => {
                    if key_start.is_some_and(|start| sensitive_query_key(&text[start..index])) {
                        return true;
                    }
                    key_start = None;
                }
                _ => {}
            }
            index += 1;
        }
        at_key_start = key_start == Some(text.len());
    }
    false
}

fn sensitive_query_key(key: &str) -> bool {
    if key.contains('%') {
        falcon_analyze::percent_decode(key).is_some_and(|decoded| sensitive(&decoded))
    } else {
        sensitive(key)
    }
}

fn sensitive(key: &str) -> bool {
    [
        "password",
        "passwd",
        "pwd",
        "token",
        "access_token",
        "refresh_token",
        "id_token",
        "api_key",
        "apikey",
        "secret",
        "client_secret",
        "authorization",
    ]
    .iter()
    .any(|candidate| key.eq_ignore_ascii_case(candidate))
}
