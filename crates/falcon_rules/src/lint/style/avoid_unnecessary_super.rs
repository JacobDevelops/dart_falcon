//! Omit implicit `super()` initializers and provably redundant member prefixes.
//!
//! Member checks require a final class or enum without library-local subclasses,
//! matching resolved read/write owners, and no shadowing lexical or library binding.
//! Open classes retain super dispatch because future subclasses can override members.
//! Constructor checks remain available without resolution.

use std::collections::HashSet;

use falcon_analyze::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, DocName, ResolvedType, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidUnnecessarySuper;

impl Rule for AvoidUnnecessarySuper {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let mut diagnostics = Vec::new();
        let mut closed = HashSet::new();
        for declaration in program.declarations() {
            match declaration {
                TopLevelDecl::Class(class)
                    if class.modifiers().restriction == Some(ClassRestriction::Final) =>
                {
                    closed.insert(class.name().name());
                }
                TopLevelDecl::Enum(enumeration) => {
                    closed.insert(enumeration.name().name());
                }
                _ => {}
            }
            let Some(members) = crate::lint::declarations::members_of(declaration) else {
                continue;
            };
            for member in members {
                let initializers = match member {
                    ClassMember::Constructor(constructor) => constructor.initializers(),
                    ClassMember::PrimaryConstructorBody(body) => body.initializers(),
                    _ => continue,
                };
                for initializer in initializers {
                    if let ConstructorInitializer::SuperCall(call) = initializer
                        && call.call_name().is_none()
                        && call.args().positional().is_empty()
                        && call.args().named().is_empty()
                    {
                        diagnostics.push(ctx.diagnostic("This empty `super()` call is implicit; remove the explicit initializer.", call.span()));
                    }
                }
            }
        }
        // Standalone analysis does not populate library-subclass facts. Exclude
        // locally declared descendants explicitly, and do not assume that part
        // libraries have been indexed completely.
        if program.part_of_directive().is_some() || program.part_directives().next().is_some() {
            closed.clear();
        } else if let Some(identities) = ctx.identities() {
            let model = falcon_analyze::SemanticModel::new(
                ctx.file_path(),
                ctx.parsed(),
                identities,
                ctx.types(),
            );
            for declaration in program.declarations() {
                let superclass = match declaration {
                    TopLevelDecl::Class(class) => class.extends(),
                    TopLevelDecl::ClassTypeAlias(alias) => Some(alias.superclass()),
                    _ => None,
                };
                if let Some(superclass) = superclass
                    && let ResolvedType::Interface { identity, .. } = model.resolve_type(superclass)
                {
                    closed.remove(identity.name());
                }
            }
        } else {
            closed.clear();
        }
        RulePreparation::Semantic(Box::new(Collector {
            ctx,
            closed,
            diagnostics,
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    closed: HashSet<&'a str>,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        let Expr::Field(field) = node else { return };
        if !matches!(field.object(), Expr::Super(_)) {
            return;
        }
        let Some(receiver @ ResolvedType::Interface { identity, .. }) = &state.enclosing_type
        else {
            return;
        };
        if !self.closed.contains(identity.name())
            || !state
                .signatures
                .declaration(identity)
                .is_some_and(|facts| !facts.has_subclass_in_library)
        {
            return;
        }
        let name = field.field().name();
        if state.environment.is_bound(name) && !state.is_own_field(name) {
            return;
        }
        // Own members precede library bindings, but inherited members do not.
        if state
            .signatures
            .declared_member_facts(identity, name)
            .is_empty()
            && !matches!(state.model.resolve_library_name(name), DocName::Absent)
        {
            return;
        }
        // Lexical declarations shadow throughout their block, even before the
        // shared semantic walk has encountered and bound them.
        if field
            .syntax()
            .ancestors()
            .take_while(|ancestor| ClassMember::cast(*ancestor).is_none())
            .filter_map(Block::cast)
            .any(|block| {
                block.stmts().any(|statement| match statement {
                    Stmt::LocalVar(declaration) => declaration
                        .declarators()
                        .any(|declarator| declarator.name().name() == name),
                    Stmt::LocalFunc(function) => function.name().name() == name,
                    Stmt::PatternDecl(declaration) => {
                        falcon_syntax::cst::visit::bound_names(declaration.pattern())
                            .iter()
                            .any(|binding| binding.name() == name)
                    }
                    _ => false,
                })
            })
        {
            return;
        }
        let inherited_read =
            state
                .signatures
                .resolved_super_member_owner(receiver, name, false, &state.model);
        let inherited_write =
            state
                .signatures
                .resolved_super_member_owner(receiver, name, true, &state.model);
        if (inherited_read.is_some() || inherited_write.is_some())
            && inherited_read
                == state
                    .signatures
                    .resolved_readable_member_owner(receiver, name, &state.model)
            && inherited_write
                == state
                    .signatures
                    .resolved_writable_member_owner(receiver, name, &state.model)
        {
            self.diagnostics.push(self.ctx.diagnostic("This inherited member does not need an explicit `super` prefix.", field.span()));
        }
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }
}
