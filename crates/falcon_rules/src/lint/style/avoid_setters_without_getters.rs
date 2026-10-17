//! Flags a setter that has no corresponding getter.
//!
//! A property that can be written but not read is surprising: `a.x = 1;`
//! compiles, but `a.x` does not. Each setter of a class, enum, or extension
//! type needs a getter of the same name, declared alongside it (a getter or a
//! field) or, except for extension types, inherited from a superclass:
//!
//! ```dart
//! class A {
//!   set width(int value) {}    // flagged
//!
//!   int get height => 0;
//!   set height(int value) {}   // fine
//! }
//! ```
//!
//! Setters that override an inherited setter are not reported. Only concrete
//! inherited getters count as a corresponding getter.
//!
//! Adopted from the official `avoid_setters_without_getters` lint.

use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, Rule, RulePreparation, StructuralRuleVisitor,
    StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use falcon_analyze::semantic_scope::SemanticState;

/// Concrete getters every type inherits from `Object`.
const OBJECT_GETTERS: [&str; 2] = ["hashCode", "runtimeType"];

#[derive(Default)]
pub struct AvoidSettersWithoutGetters;

impl Rule for AvoidSettersWithoutGetters {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let Some(state) = SemanticState::new(program, ctx) else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        RulePreparation::Structural(Box::new(Collector {
            program,
            ctx,
            state,
            diags: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    program: Program<'a>,
    ctx: &'a AnalyzeContext<'a>,
    state: SemanticState<'a>,
    diags: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_top_level_decl(&mut self, declaration: TopLevelDecl<'a>) -> StructuralTraversal {
            let (name, members) = match declaration {
                TopLevelDecl::Class(node) => (node.name(), node.members()),
                TopLevelDecl::MixinClass(node) => (node.name(), node.members()),
                TopLevelDecl::Enum(node) => (node.name(), node.members()),
                TopLevelDecl::ExtensionType(node) => (node.name(), node.members()),
                TopLevelDecl::ClassTypeAlias(_)
                | TopLevelDecl::Mixin(_)
                | TopLevelDecl::Extension(_)
                | TopLevelDecl::Function(_)
                | TopLevelDecl::Variable(_)
                | TopLevelDecl::TypeAlias(_)
                | TopLevelDecl::Error(_) => return StructuralTraversal::SkipChildren,
            };
            check_members(
                name.name(),
                members,
                self.program,
                matches!(declaration, TopLevelDecl::ExtensionType(_)),
                &self.state,
                self.ctx,
                &mut self.diags,
            );
        StructuralTraversal::SkipChildren
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }
}
}

fn declares_getter<'a>(mut members: AstChildren<'a, ClassMember<'a>>, name: &str) -> bool {
    members.any(|member| match member {
        ClassMember::Getter(getter) => getter.name().name() == name,
        ClassMember::Field(field) => field
            .declarators()
            .any(|declarator| declarator.name().name() == name),
        ClassMember::Constructor(_)
        | ClassMember::PrimaryConstructorBody(_)
        | ClassMember::Method(_)
        | ClassMember::Setter(_)
        | ClassMember::Operator(_)
        | ClassMember::Error(_) => false,
    })
}

fn check_members<'a>(
    owner: &str,
    members: AstChildren<'a, ClassMember<'a>>,
    program: Program<'a>,
    extension_type: bool,
    state: &SemanticState<'_>,
    ctx: &AnalyzeContext,
    diags: &mut Vec<Diagnostic>,
) {
    let owner = state.model.resolve_name(owner);
    for member in members.clone() {
        let ClassMember::Setter(setter) = member else {
            continue;
        };
        let name = setter.name().name();
        let overrides = setter.annotations().any(|annotation| {
            annotation
                .name()
                .last()
                .is_some_and(|n| n.name() == "override")
        });
        if overrides || declares_getter(members.clone(), name) || OBJECT_GETTERS.contains(&name) {
            continue;
        }
        let inherited = owner
            .as_ref()
            .filter(|_| !extension_type)
            .and_then(|owner| state.signatures.inherited_member_facts(owner, name))
            .unwrap_or_default();
        let has_inherited = inherited
            .iter()
            .any(|fact| fact.kind.has_setter() || (fact.kind.has_getter() && !fact.is_abstract
                && !program.declarations().any(|declaration| {
                    let (declared_name, mut declared_members) = match declaration {
                        TopLevelDecl::Class(class) => (class.name(), class.members()),
                        TopLevelDecl::MixinClass(class) => (class.name(), class.members()),
                        TopLevelDecl::Mixin(mixin) => (mixin.name(), mixin.members()),
                        _ => return false,
                    };
                    matches!((owner.as_ref(), &fact.declaring_type),
                        (Some(DeclarationIdentity::Project { library: own_library, .. }),
                         DeclarationIdentity::Project { library, name: declaring_name })
                            if own_library == library && declaring_name == declared_name.name())
                        && declared_members.any(|member| matches!(member,
                            ClassMember::Getter(getter) if getter.name().name() == name && getter.body().is_none()))
                })));
        if !has_inherited {
            diags.push(ctx.diagnostic(
                "avoid-setters-without-getters",
                "Setter has no corresponding getter.",
                setter.name().span(),
            ));
        }
    }
}
