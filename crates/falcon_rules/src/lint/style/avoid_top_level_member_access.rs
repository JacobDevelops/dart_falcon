//! Flags mutable top-level variables and static fields, plus references to mutable
//! top-level variables declared in this file outside top-level initializers.
//!
//! Declaration and reference checks use syntax and lexical bindings only; enabling
//! this rule does not request project-wide resolution. Locals and members declared
//! on the enclosing type shadow globals; inherited members do not.
//!
//! Mutable top-level and static members are shared, unscoped state that any
//! code can read or write, which makes data flow hard to follow and tests hard
//! to isolate. Prefer `const` or `final` values, or pass state explicitly
//! through constructors and parameters.

use std::collections::{HashMap, HashSet};

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidTopLevelMemberAccess;

impl Rule for AvoidTopLevelMemberAccess {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let mut top_level_vars = HashSet::new();
        let mut initializers = Vec::new();
        let mut diags = Vec::new();
        for declaration in program.declarations() {
            if let TopLevelDecl::Variable(var) = declaration {
                initializers.push(var.span());
                if var.keyword() == VarKeyword::Var {
                    diags.push(ctx.diagnostic(MESSAGE, var.span()));
                    top_level_vars.extend(var.declarators().map(|decl| decl.name().name()));
                }
            }
            let members = match declaration {
                TopLevelDecl::Class(node) => Some(node.members()),
                TopLevelDecl::Mixin(node) => Some(node.members()),
                TopLevelDecl::MixinClass(node) => Some(node.members()),
                TopLevelDecl::Enum(node) => Some(node.members()),
                TopLevelDecl::Extension(node) => Some(node.members()),
                TopLevelDecl::ExtensionType(node) => Some(node.members()),
                _ => None,
            };
            for member in members.into_iter().flatten() {
                if let ClassMember::Field(field) = member
                    && field.is_static()
                    && !matches!(field.keyword(), VarKeyword::Final | VarKeyword::Const)
                {
                    diags.push(ctx.diagnostic(MESSAGE, field.span()));
                }
            }
        }
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            top_level_vars,
            initializers,
            local_declarations: ctx.local_declarations(program),
            diags,
            member_names: HashSet::new(),
        }))
    }
}

const MESSAGE: &str = "Avoid using non-const top-level members.";

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    top_level_vars: HashSet<&'a str>,
    initializers: Vec<Span>,
    local_declarations: &'a HashMap<usize, NodeId>,
    diags: Vec<Diagnostic>,
    member_names: HashSet<&'a str>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_top_level_decl(&mut self, node: TopLevelDecl<'a>) -> StructuralTraversal {
        self.member_names.clear();
        if let Some(members) = crate::lint::declarations::members_of(node) {
            self.member_names = crate::lint::declarations::member_names(members.clone());
            let effective = match node {
                TopLevelDecl::Class(node) => node.element_members(),
                TopLevelDecl::MixinClass(node) => node.element_members(),
                TopLevelDecl::Enum(node) => node.element_members(),
                TopLevelDecl::ExtensionType(node) => node.element_members(),
                _ => EffectiveMembers::declared(members),
            };
            self.member_names
                .extend(effective.declaring_params().map(|param| param.name().name()));
        }
        StructuralTraversal::Continue
    }

    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        if let Expr::Ident(name) = node
            && self.top_level_vars.contains(name.name())
            && !self.initializers.iter().any(|span| span.contains_offset(name.span().start))
            && !self.local_declarations.contains_key(&name.span().start)
            && !self.member_names.contains(name.name())
        {
            self.diags.push(self.ctx.diagnostic(MESSAGE, name.span()));
        }
        StructuralTraversal::Continue
    }
}
}

#[cfg(test)]
mod tests {
    use super::*;
    use falcon_analyze::RuleAnalysis;

    #[test]
    fn unresolved_context_keeps_declarations_and_reference_diagnostics() {
        let source =
            "var counter = 0; class C { static int count = 0; } void f() { print(counter); }";
        let parsed = std::sync::Arc::new(falcon_analyze::ParsedFile::parse(source));
        let config = falcon_config::FalconConfig::default();
        let ctx = AnalyzeContext::new(std::path::Path::new("unresolved.dart"), &config, &parsed);
        let diagnostics = AvoidTopLevelMemberAccess.analyze(
            "avoid-top-level-member-access",
            parsed.program(),
            &ctx,
        );
        assert_eq!(diagnostics.len(), 3, "{diagnostics:?}");
    }

    #[test]
    fn does_not_require_project_resolution() {
        assert!(!crate::rule_requires_resolution(
            "avoid-top-level-member-access"
        ));
    }

    #[test]
    fn declared_members_shadow_globals_but_inherited_members_do_not() {
        let source = r"
var counter = 0;
class Base { int counter = 1; }
class Child extends Base { int read() => counter; }
class Other { int read() => counter; }
void readGlobal() { print(counter); }
";
        let diagnostics = crate::lint::test_support::analyze_flutter_rule(
            "avoid-top-level-member-access",
            source,
            &AvoidTopLevelMemberAccess,
        );
        assert_eq!(diagnostics.len(), 4, "{diagnostics:?}");
    }
}
