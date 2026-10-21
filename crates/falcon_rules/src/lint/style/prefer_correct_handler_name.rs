//! Match callback handler methods to a configured regular expression.
//!
//! Nonempty valid regex `name-pattern` enables the rule. `ignore-public`
//! (default false) exempts public methods. Direct method tear-offs passed as
//! positional or named arguments are checked, including unqualified references
//! unless shadowed by a lexical binding. Only ordinary unannotated methods
//! declared on an enclosing class without supertypes are checked. Global
//! functions, arbitrary object methods and closure wrappers are excluded.
//! Leading privacy underscores are ignored.

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, ResolvedType, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;
use crate::lint::options;
use std::collections::HashMap;

#[derive(Default, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
#[serde(default, rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) struct Options {
    /// Regular expression matching handler method names; absent or empty patterns disable this rule.
    name_pattern: Option<options::Regex>,
    /// Whether to skip public handler methods.
    ignore_public: bool,
}

crate::lint::options::impl_options!(Options);

#[derive(Default)]
pub struct PreferCorrectHandlerName;
impl Rule for PreferCorrectHandlerName {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let options = options::get::<Options>(ctx, Self::NAME);
        let Some(pattern) = options
            .name_pattern
            .as_ref()
            .filter(|pattern| !pattern.as_str().is_empty())
        else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        RulePreparation::Semantic(Box::new(Collector {
            ctx,
            program,
            pattern,
            ignore_public: options.ignore_public,
            reported: Vec::new(),
            diags: Vec::new(),
        }))
    }
}
struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    program: Program<'a>,
    pattern: &'a options::Regex,
    ignore_public: bool,
    reported: Vec<Span>,
    diags: Vec<Diagnostic>,
}
impl<'a> Collector<'a> {
    fn check(&mut self, value: Expr<'a>, class: ClassDecl<'a>, state: &SemanticState<'a>) {
        let name = match value.unparenthesized() {
            Expr::Ident(name) if !state.environment.is_bound(name.name()) => name,
            Expr::Field(field) if matches!(field.object().unparenthesized(), Expr::This(_)) => {
                field.field()
            }
            _ => return,
        };
        if self.pattern.is_match(name.name().trim_start_matches('_'))
            || (self.ignore_public && !name.name().starts_with('_'))
        {
            return;
        }
        let method = class.members().find_map(|member| match member {
            ClassMember::Method(method)
                if method.name().name() == name.name()
                    && !method.is_static()
                    && method.annotations().is_empty() =>
            {
                Some(method)
            }
            _ => None,
        });
        if let Some(method) = method {
            let span = method.name().span();
            if self.reported.contains(&span) {
                return;
            }
            self.reported.push(span);
            self.diags.push(self.ctx.diagnostic(format!("This callback handler does not match '{}'; rename it to match the configured pattern.", self.pattern.as_str()), span));
        }
    }
}
impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }
    fn visit_call_arguments(
        &mut self,
        args: ArgList<'a>,
        _: &[ResolvedType],
        _: &HashMap<String, ResolvedType>,
        state: &SemanticState<'a>,
    ) {
        let span = args.span();
        let class = self
            .program
            .declarations()
            .find_map(|declaration| match declaration {
                TopLevelDecl::Class(class)
                    if class.span().start <= span.start && class.span().end >= span.end =>
                {
                    Some(class)
                }
                _ => None,
            });
        let Some(class) = class else {
            return;
        };
        if class.extends().is_some()
            || !class.with_clause().is_empty()
            || !class.implements().is_empty()
        {
            return;
        }
        for value in args.positional().chain(args.named().map(|arg| arg.value())) {
            self.check(value, class, state);
        }
    }
}
