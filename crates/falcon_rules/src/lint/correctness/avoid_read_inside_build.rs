//! Flags non-listening Provider reads directly inside Flutter widget builds.
//!
//! A read does not rebuild the widget when its value changes. This checks
//! package:provider context extensions and `Provider.of(listen: false)` on a
//! Flutter BuildContext. Nested callbacks are skipped: their execution phase
//! depends on the receiving API, not their lexical position. BuildWithChild
//! overrides on nested's single-child widgets are also build entry points;
//! didChangeDependencies permits subscriptions but is not a build.

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, ResolvedType, Rule, RulePreparation, Truth,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidReadInsideBuild;

impl Rule for AvoidReadInsideBuild {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        prepare(program, ctx, Check::ReadInsideBuild)
    }
}

pub(super) enum Check {
    ReadInsideBuild,
    WatchOutsideBuild,
    ReadForMethods,
}

pub(super) fn prepare<'a>(
    program: Program<'a>,
    ctx: &'a AnalyzeContext<'a>,
    check: Check,
) -> RulePreparation<'a> {
    let extensions = program.imports().any(|import| {
        import.uri().value().starts_with("package:provider/")
            && import.as_name().is_none()
            && import.combinators().is_empty()
            && import.configurable_uris().is_empty()
    });
    RulePreparation::Semantic(Box::new(Collector {
        ctx,
        check,
        extensions,
        diagnostics: Vec::new(),
    }))
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    check: Check,
    extensions: bool,
    diagnostics: Vec<Diagnostic>,
}

impl Collector<'_> {
    fn access(&self, expression: Expr<'_>, state: &SemanticState<'_>) -> Option<bool> {
        let Expr::Call(call) = expression else {
            return None;
        };
        let Expr::Field(field) = call.callee().unparenthesized() else {
            return None;
        };
        let method = field.field().name();
        if self.extensions && matches!(method, "read" | "watch" | "select") {
            let receiver = state.infer(field.object());
            if matches!(&receiver, ResolvedType::Interface { identity, .. }
                if identity.is_package("flutter", "BuildContext"))
                && state
                    .signatures
                    .resolved_member_owner(&receiver, method, &state.model)
                    .is_none()
            {
                if let Some((identity, _, _)) =
                    state
                        .signatures
                        .extension_method(&receiver, method, &state.model)
                    && !identity.is_some_and(|identity| provider_identity(identity, state))
                {
                    return None;
                }
                return Some(method != "read");
            }
        }
        if method != "of" {
            return None;
        }
        let segments = falcon_analyze::expression_segments(field.object(), false)?;
        if segments
            .first()
            .is_some_and(|head| state.environment.is_bound(head))
        {
            return None;
        }
        let owner = state.model.resolve_dotted(&segments)?;
        if owner.name() != "Provider" || !provider_identity(&owner, state) {
            return None;
        }
        let receiver = state.infer(call.args().positional().next()?);
        if !matches!(receiver, ResolvedType::Interface { identity, .. }
            if identity.is_package("flutter", "BuildContext"))
        {
            return None;
        }
        match call
            .args()
            .named()
            .find(|arg| arg.name().name() == "listen")
        {
            None => Some(true),
            Some(argument) => match argument.value().unparenthesized() {
                Expr::BoolLit(value) => Some(value.value()),
                _ => None,
            },
        }
    }
}

fn provider_identity(identity: &DeclarationIdentity, state: &SemanticState<'_>) -> bool {
    matches!(identity, DeclarationIdentity::Package { package, .. } if package.as_str() == "provider")
        || state
            .model
            .declaration_uri(identity)
            .is_some_and(|uri| uri.starts_with("package:provider/"))
}
fn inside_build(expression: Expr<'_>, state: &SemanticState<'_>) -> Option<bool> {
    for ancestor in expression.syntax().ancestors() {
        if let Some(function) = FuncExpr::cast(ancestor) {
            for parent in function.syntax().ancestors().skip(1) {
                if ParenExpr::cast(parent).is_some() {
                    continue;
                }
                if let Some(argument) = NamedArg::cast(parent) {
                    return argument.name().name().starts_with("on").then_some(false);
                }
                if ArgList::cast(parent).is_some() {
                    return None;
                }
                break;
            }
            return Some(false);
        }
        if FunctionDecl::cast(ancestor).is_some() || LocalFuncDecl::cast(ancestor).is_some() {
            return Some(false);
        }
        if let Some(method) = MethodDecl::cast(ancestor) {
            if method.is_static() || matches!(method.name().name(), "initState" | "dispose") {
                return Some(false);
            }
            if method
                .syntax()
                .ancestors()
                .find_map(ClassDecl::cast)
                .is_some_and(|class| {
                    class.extends().is_none()
                        && class.with_clause().is_empty()
                        && class.implements().is_empty()
                })
            {
                return Some(false);
            }
            let owner = state.enclosing_type.as_ref()?;
            let targets = match method.name().name() {
                "build" => [
                    DeclarationIdentity::flutter("StatelessWidget"),
                    DeclarationIdentity::flutter("State"),
                ],
                "buildWithChild" => [
                    DeclarationIdentity::package("nested", "SingleChildStatelessWidget"),
                    DeclarationIdentity::package("nested", "SingleChildState"),
                ],
                // Helpers may run synchronously from build, and
                // didChangeDependencies explicitly permits dependencies.
                _ => return None,
            };
            return targets
                .iter()
                .any(|target| {
                    state.signatures.is_subtype_of(owner, target, &state.model) == Truth::Yes
                })
                .then_some(true);
        }
        if let Some(class) = ClassDecl::cast(ancestor) {
            return (class.extends().is_none()
                && class.with_clause().is_empty()
                && class.implements().is_empty())
            .then_some(false);
        }
    }
    state.enclosing_type.is_none().then_some(false)
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }

    fn visit_expr(&mut self, expression: Expr<'a>, state: &SemanticState<'a>) {
        let message = match self.check {
            Check::ReadInsideBuild
                if self.access(expression, state) == Some(false)
                    && inside_build(expression, state) == Some(true) =>
            {
                "Use watch instead of read for values used during build."
            }
            Check::WatchOutsideBuild
                if self.access(expression, state) == Some(true)
                    && inside_build(expression, state) == Some(false) =>
            {
                "Use read instead of subscribing outside build."
            }
            Check::ReadForMethods => {
                let Expr::Call(call) = expression else {
                    return;
                };
                let Expr::Field(method) = call.callee().unparenthesized() else {
                    return;
                };
                if self.access(method.object(), state) != Some(true)
                    || inside_build(expression, state) != Some(true)
                {
                    return;
                }
                if let Expr::Call(access) = method.object().unparenthesized()
                    && let Expr::Field(lookup) = access.callee().unparenthesized()
                    && lookup.field().name() == "select"
                {
                    return;
                }
                let mut receiver = state.infer(method.object());
                if let Expr::Call(access) = method.object().unparenthesized()
                    && let Some(ty) = access.type_args().last()
                {
                    receiver = state.model.resolve_type(ty);
                }
                let Some((signature, _)) = state.signatures.resolved_member(
                    &receiver,
                    method.field().name(),
                    &state.model,
                ) else {
                    return;
                };
                if signature.return_type != ResolvedType::Void {
                    return;
                }
                "Use read instead of watch when invoking a provided method."
            }
            _ => return,
        };
        self.diagnostics
            .push(self.ctx.diagnostic(message, expression.span()));
    }
}
