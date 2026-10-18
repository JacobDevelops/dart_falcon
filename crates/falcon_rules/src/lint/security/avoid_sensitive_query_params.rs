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
                for (segment, _) in literal.segments() {
                    let text = segment.content.split('#').next().unwrap_or("");
                    if (text.starts_with("https://") || text.starts_with("http://"))
                        && let Some((_, query)) = text.split_once('?')
                        && query
                            .split('&')
                            .any(|pair| pair.split_once('=').is_some_and(|(key, _)| sensitive(key)))
                    {
                        self.report(node.span());
                        break;
                    }
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
