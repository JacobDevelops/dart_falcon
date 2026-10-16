//! Flags leading constructor-body asserts that could live in the initializer list.
//!
//! An `assert` at the start of a generative constructor body that does not
//! need the instance (no `this`, no instance member of the class or its
//! superclasses and mixins) can move into the initializer list, where it runs
//! before the object is used and works for `const` constructors too. Fields
//! initialized by `this.x` formals count as available, since the formal holds
//! the same value, as do fields a `super.x` formal passes to a superclass
//! constructor's `this.x` parameter. The analyzer retains the last class's
//! superclass/mixin chain for subsequent extension type constructors.
//!
//! ```dart
//! class A {
//!   A(int a) { assert(a != 0); } // flagged
//!   A(int a) : assert(a != 0);   // ok
//! }
//! ```
//!
//! Adopted from the official `prefer_asserts_in_initializer_lists` lint.

use std::collections::{HashMap, HashSet};

use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, ResolvedType, Rule, SemanticModel, SignatureIndex,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::for_each_expr;

#[derive(Default)]
pub struct PreferAssertsInInitializerLists;

impl Rule for PreferAssertsInInitializerLists {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(model) = ctx
            .identities
            .map(|identities| SemanticModel::new(ctx.file_path, identities, ctx.types))
        else {
            return Vec::new();
        };
        let Some(signatures) = ctx.signatures else {
            return Vec::new();
        };
        let constructors = constructor_parameters(program, &model);
        let mut diags = Vec::new();
        let mut cached_class = None;
        for declaration in &program.declarations {
            let (name, primary, members) = match declaration {
                TopLevelDecl::Class(class) => (
                    &class.name.name,
                    class.primary_constructor.as_ref(),
                    &class.members,
                ),
                TopLevelDecl::MixinClass(class) => (
                    &class.name.name,
                    class.primary_constructor.as_ref(),
                    &class.members,
                ),
                TopLevelDecl::Enum(class) => (
                    &class.name.name,
                    class.primary_constructor.as_ref(),
                    &class.members,
                ),
                TopLevelDecl::ExtensionType(class) => (
                    &class.name.name,
                    Some(&class.primary_constructor),
                    &class.members,
                ),
                _ => continue,
            };
            let Some(identity) = model.resolve_name(name) else {
                continue;
            };
            let receiver = ResolvedType::interface_identity(identity.clone());
            // The analyzer caches class elements, not names. Extension type
            // constructors retain that cache but resolve their own members.
            if !matches!(declaration, TopLevelDecl::ExtensionType(_)) {
                cached_class = Some(receiver.clone());
            }
            for member in members {
                let (params, initializers, body) = match member {
                    ClassMember::Constructor(constructor) if !constructor.is_factory => (
                        &constructor.params,
                        &constructor.initializers,
                        constructor.body.as_ref(),
                    ),
                    ClassMember::PrimaryConstructorBody(body) => {
                        let Some(primary) = primary else { continue };
                        (&primary.params, &body.initializers, body.body.as_ref())
                    }
                    _ => continue,
                };
                let Some(FunctionBody::Block(block)) = body else {
                    continue;
                };
                let super_params = signatures
                    .superclass(&identity)
                    .and_then(|superclass| match superclass {
                        ResolvedType::Interface { identity, .. } => Some(identity),
                        _ => None,
                    })
                    .and_then(|superclass| {
                        let invoked = initializers
                            .iter()
                            .find_map(|initializer| match initializer {
                                ConstructorInitializer::SuperCall { call_name, .. } => Some(
                                    call_name.as_ref().map_or("new", |name| name.name.as_str()),
                                ),
                                _ => None,
                            })
                            .unwrap_or("new");
                        constructors.get(superclass)?.get(invoked).copied()
                    });
                let scope = ConstructorScope::new(params, super_params);
                for statement in &block.stmts {
                    let Stmt::Assert(assert) = statement else {
                        break;
                    };
                    if !needs_instance(
                        assert,
                        &scope,
                        &receiver,
                        cached_class.as_ref(),
                        &model,
                        signatures,
                    ) {
                        diags.push(ctx.diagnostic(
                            "prefer-asserts-in-initializer-lists",
                            "Assert should be in the initializer list.",
                            Span::new(assert.span.start, assert.span.start + "assert".len()),
                        ));
                    }
                }
            }
        }
        diags
    }
}

/// Keep syntax only for initializing formals, which resolved signatures do not
/// distinguish from ordinary parameters. Member lookup uses declaration identities.
fn constructor_parameters<'p>(
    program: &'p Program,
    model: &SemanticModel<'_>,
) -> HashMap<DeclarationIdentity, HashMap<&'p str, &'p FormalParamList>> {
    program
        .declarations
        .iter()
        .filter_map(|declaration| {
            let (name, primary, members) = match declaration {
                TopLevelDecl::Class(node) => {
                    (&node.name, node.primary_constructor.as_ref(), &node.members)
                }
                TopLevelDecl::MixinClass(node) => {
                    (&node.name, node.primary_constructor.as_ref(), &node.members)
                }
                TopLevelDecl::Enum(node) => {
                    (&node.name, node.primary_constructor.as_ref(), &node.members)
                }
                _ => return None,
            };
            let identity = model.resolve_name(&name.name)?;
            let primary = primary.map(|primary| {
                (
                    primary
                        .constructor_name
                        .as_ref()
                        .map_or("new", |name| name.name.as_str()),
                    &primary.params,
                )
            });
            let constructors = members
                .iter()
                .filter_map(|member| match member {
                    ClassMember::Constructor(constructor) if !constructor.is_factory => Some((
                        constructor
                            .constructor_name
                            .as_ref()
                            .map_or("new", |name| name.name.as_str()),
                        &constructor.params,
                    )),
                    _ => None,
                })
                .chain(primary)
                .collect();
            Some((identity, constructors))
        })
        .collect()
}

/// Names a constructor body sees without needing the instance.
struct ConstructorScope<'p> {
    /// Regular (non-initializing) formals, which shadow members.
    params: HashSet<&'p str>,
    /// Fields initialized by `this.x` formals: own ones, and the superclass's
    /// reached through a `super.x` formal.
    initialized_fields: HashSet<&'p str>,
}

impl<'p> ConstructorScope<'p> {
    fn new(params: &'p FormalParamList, super_params: Option<&'p FormalParamList>) -> Self {
        let mut scope = Self {
            params: HashSet::new(),
            initialized_fields: HashSet::new(),
        };
        let mut super_positional = 0;
        for param in &params.params {
            let name = param.name.name.as_str();
            match param.initializing {
                None => {
                    scope.params.insert(name);
                }
                Some(InitializingFormal::This) => {
                    scope.initialized_fields.insert(name);
                }
                Some(InitializingFormal::Super) => {
                    // The super constructor parameter this formal forwards to:
                    // by position, or by name for a named formal.
                    let named = matches!(param.kind, ParamKind::Named { .. });
                    let target = super_params.and_then(|super_params| {
                        if named {
                            super_params.params.iter().find(|candidate| {
                                matches!(candidate.kind, ParamKind::Named { .. })
                                    && candidate.name.name == name
                            })
                        } else {
                            super_params
                                .params
                                .iter()
                                .filter(|candidate| {
                                    !matches!(candidate.kind, ParamKind::Named { .. })
                                })
                                .nth(super_positional)
                        }
                    });
                    if !named {
                        super_positional += 1;
                    }
                    if let Some(target) = target
                        && target.initializing == Some(InitializingFormal::This)
                    {
                        scope.initialized_fields.insert(target.name.name.as_str());
                    }
                }
            }
        }
        scope
    }
}

fn needs_instance(
    assert: &AssertStmt,
    scope: &ConstructorScope<'_>,
    receiver: &ResolvedType,
    cached_class: Option<&ResolvedType>,
    model: &SemanticModel<'_>,
    signatures: &SignatureIndex,
) -> bool {
    let mut closure_params = HashSet::new();
    let mut identifiers = Vec::new();
    let mut properties = Vec::new();
    let mut uses_receiver = false;
    let mut collect = |expression: &Expr| match expression.unparenthesized() {
        Expr::This { .. } | Expr::Super { .. } => uses_receiver = true,
        Expr::Ident(identifier) => identifiers.push(identifier.name.clone()),
        Expr::Field { field, .. } => properties.push(field.name.clone()),
        Expr::FuncExpr { params, .. } => {
            closure_params.extend(params.params.iter().map(|p| p.name.name.clone()));
        }
        Expr::Cascade { sections, .. } => {
            for op in sections.iter().flat_map(|section| &section.ops) {
                match op {
                    CascadeOp::Field { name, .. } | CascadeOp::Call(name, ..) => {
                        properties.push(name.name.clone());
                    }
                    CascadeOp::Invoke(..)
                    | CascadeOp::NullAssert { .. }
                    | CascadeOp::Index { .. }
                    | CascadeOp::Assign(..)
                    | CascadeOp::IndexAssign(..) => {}
                }
            }
        }
        _ => {}
    };
    for_each_expr(&assert.condition, &mut collect);
    if let Some(message) = &assert.message {
        for_each_expr(message, &mut collect);
    }
    if uses_receiver {
        return true;
    }
    let is_cached_member = |name: &str| {
        // Assertions read getters, fields and tear-offs; inherited getters
        // need not have an invocable interface member.
        let Some(cached) = cached_class else {
            return false;
        };
        let cached_owner = signatures.resolved_readable_member_owner(cached, name, model);
        // Extension type constructors retain the preceding class's inventory.
        // An unknown lookup on the extension must not invent a member in that
        // unrelated cached class (or before any class has been visited).
        if cached != receiver && cached_owner.is_none() {
            return false;
        }
        let Some(owner) = signatures.resolved_readable_member_owner(receiver, name, model) else {
            // An absent owner only proves absence when the inherited namespace
            // is complete. Unloaded dependencies may still provide this name.
            let ResolvedType::Interface { identity, .. } = receiver else {
                return true;
            };
            return signatures
                .known_inherited_member_facts(identity, name)
                .is_none_or(|(facts, complete)| !facts.is_empty() || !complete);
        };
        cached_owner.as_ref() == Some(&owner)
    };
    // A selector may resolve to a member of this class on another receiver.
    if properties.iter().any(|name| is_cached_member(name)) {
        return true;
    }
    identifiers.iter().any(|name| {
        let name = name.as_str();
        !closure_params.contains(name)
            && !scope.params.contains(name)
            && is_cached_member(name)
            && !scope.initialized_fields.contains(name)
    })
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use falcon_analyze::{IdentityIndex, IdentitySource};
    use falcon_config::FalconConfig;
    use falcon_dart_parser::parse;

    use super::*;

    #[test]
    fn reports_only_the_assert_keyword() {
        let source = "class Box { Box(int value) { assert /* gap */ (value > 0); assert(value < 10, 'limit'); } }";
        let path = Path::new("/project/lib/test.dart");
        let falcon_dart_parser::ParseResult {
            program,
            errors,
            syntax,
        } = parse(source);
        assert!(errors.is_empty(), "{errors:?}");
        let identities = IdentityIndex::from_project_files(
            &[IdentitySource {
                path,
                program: &program,
                has_parse_errors: false,
            }],
            &[],
        );
        let model = SemanticModel::new(path, &identities, None);
        let signatures = SignatureIndex::from_program(&program, &model);
        let config = FalconConfig::default();
        let ctx = AnalyzeContext::new(path, source, &config, &syntax)
            .with_identities(&identities)
            .with_signatures(&signatures);
        let diagnostics = PreferAssertsInInitializerLists.analyze(&program, &ctx);
        let slices: Vec<_> = diagnostics
            .iter()
            .map(|diagnostic| &source[diagnostic.span.start..diagnostic.span.end])
            .collect();
        assert_eq!(slices, ["assert", "assert"]);
    }
}
