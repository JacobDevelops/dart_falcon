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
    AnalyzeContext, DeclarationIdentity, ResolvedType, Rule, RulePreparation, SemanticModel,
    SignatureIndex, StructuralNode, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct PreferAssertsInInitializerLists;

impl Rule for PreferAssertsInInitializerLists {
    fn prepare<'a>(
        &self,
        program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let Some(model) = ctx
            .identities
            .map(|identities| SemanticModel::new(ctx.file_path, identities, ctx.types))
        else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        let Some(signatures) = ctx.signatures else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        let constructors = constructor_parameters(program, &model);
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            model,
            signatures,
            constructors,
            receiver: None,
            cached_class: None,
            scope: None,
            primary_scope: None,
            leading_end: 0,
            primary_end: 0,
            pending: None,
            diagnostics: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    model: SemanticModel<'a>,
    signatures: &'a SignatureIndex,
    constructors: HashMap<DeclarationIdentity, HashMap<&'a str, &'a FormalParamList>>,
    receiver: Option<ResolvedType>,
    cached_class: Option<ResolvedType>,
    scope: Option<ConstructorScope<'a>>,
    primary_scope: Option<ConstructorScope<'a>>,
    leading_end: usize,
    primary_end: usize,
    pending: Option<AssertionFacts<'a>>,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> Collector<'a> {
    fn constructor_scope(
        &self,
        params: &FormalParamList,
        initializers: &[ConstructorInitializer],
    ) -> ConstructorScope<'a> {
        let super_params = self
            .receiver
            .as_ref()
            .and_then(|receiver| match receiver {
                ResolvedType::Interface { identity, .. } => self.signatures.superclass(identity),
                _ => None,
            })
            .and_then(|superclass| match superclass {
                ResolvedType::Interface { identity, .. } => Some(identity),
                _ => None,
            })
            .and_then(|superclass| {
                let invoked = initializers
                    .iter()
                    .find_map(|initializer| match initializer {
                        ConstructorInitializer::SuperCall { call_name, .. } => {
                            Some(call_name.as_ref().map_or("new", |name| name.name.as_str()))
                        }
                        _ => None,
                    })
                    .unwrap_or("new");
                self.constructors.get(superclass)?.get(invoked).copied()
            });
        ConstructorScope::new(params, super_params, self.ctx.source)
    }
}

fn leading_assert_end(body: Option<&FunctionBody>) -> usize {
    let Some(FunctionBody::Block(block)) = body else {
        return 0;
    };
    block
        .stmts
        .iter()
        .take_while(|statement| matches!(statement, Stmt::Assert(_)))
        .last()
        .map_or(0, |statement| statement.span().end)
}

falcon_analyze::structural_visitor! {
impl StructuralRuleVisitor for Collector<'_> {
    fn visit_top_level_decl(&mut self, declaration: &TopLevelDecl) -> StructuralTraversal {
        let name = match declaration {
            TopLevelDecl::Class(class) => &class.name.name,
            TopLevelDecl::MixinClass(class) => &class.name.name,
            TopLevelDecl::Enum(class) => &class.name.name,
            TopLevelDecl::ExtensionType(class) => &class.name.name,
            _ => return StructuralTraversal::SkipChildren,
        };
        let Some(identity) = self.model.resolve_name(name) else {
            return StructuralTraversal::SkipChildren;
        };
        let receiver = ResolvedType::interface_identity(identity);
        if !matches!(declaration, TopLevelDecl::ExtensionType(_)) {
            self.cached_class = Some(receiver.clone());
        }
        self.receiver = Some(receiver);
        self.primary_scope = None;
        self.scope = None;
        self.leading_end = 0;
        StructuralTraversal::Continue
    }

    fn visit_constructor(&mut self, constructor: ConstructorRef<'_>) -> StructuralTraversal {
        if constructor.is_primary {
            self.primary_end = leading_assert_end(constructor.body);
            self.primary_scope = (self.primary_end > 0).then(|| {
                self.constructor_scope(constructor.params, constructor.initializers)
            });
        }
        StructuralTraversal::Continue
    }

    fn visit_class_member(&mut self, member: &ClassMember) -> StructuralTraversal {
        match member {
            ClassMember::Constructor(constructor) if !constructor.is_factory => {
                self.leading_end = leading_assert_end(constructor.body.as_ref());
                self.scope = (self.leading_end > 0).then(|| {
                    self.constructor_scope(&constructor.params, &constructor.initializers)
                });
            }
            ClassMember::PrimaryConstructorBody(_) => {
                self.scope = self.primary_scope.take();
                self.leading_end = self.primary_end;
            }
            _ => return StructuralTraversal::SkipChildren,
        }
        StructuralTraversal::Continue
    }

    fn route_child(&mut self, parent: StructuralNode<'_>, child: StructuralNode<'_>) -> bool {
        match parent {
            StructuralNode::ConstructorDecl(_) | StructuralNode::PrimaryConstructorBody(_) => {
                matches!(child, StructuralNode::FunctionBody(_))
            }
            _ => true,
        }
    }

    fn exit_class_member(&mut self, _member: &ClassMember) {
        self.scope = None;
        self.leading_end = 0;
    }

    fn visit_assert_stmt(&mut self, assertion: &AssertStmt) -> StructuralTraversal {
        if self.pending.is_none() && self.scope.is_some() && assertion.span.end <= self.leading_end {
            self.pending = Some(AssertionFacts {
                span: assertion.span,
                closure_params: HashSet::new(),
                identifiers: Vec::new(),
                properties: Vec::new(),
                uses_receiver: false,
            });
        }
        StructuralTraversal::Continue
    }

    fn visit_expr(&mut self, expression: &Expr) -> StructuralTraversal {
        let Some(facts) = &mut self.pending else {
            return StructuralTraversal::SkipChildren;
        };
        facts.record(expression, self.ctx.source);
        StructuralTraversal::Continue
    }

    fn exit_assert_stmt(&mut self, assertion: &AssertStmt) {
        if self.pending.as_ref().is_none_or(|facts| facts.span != assertion.span) {
            return;
        }
        let facts = self.pending.take().expect("active assertion");
        if !needs_instance(
            &facts,
            self.scope.as_ref().expect("constructor scope"),
            self.receiver.as_ref().expect("constructor receiver"),
            self.cached_class.as_ref(),
            &self.model,
            self.signatures,
        ) {
            self.diagnostics.push(self.ctx.diagnostic(
                "prefer-asserts-in-initializer-lists",
                "Assert should be in the initializer list.",
                Span::new(assertion.span.start, assertion.span.start + "assert".len()),
            ));
        }
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }
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
    fn new(
        params: &FormalParamList,
        super_params: Option<&FormalParamList>,
        source: &'p str,
    ) -> Self {
        let mut scope = Self {
            params: HashSet::new(),
            initialized_fields: HashSet::new(),
        };
        let mut super_positional = 0;
        for param in &params.params {
            let name = &source[param.name.span.start..param.name.span.end];
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
                        scope
                            .initialized_fields
                            .insert(&source[target.name.span.start..target.name.span.end]);
                    }
                }
            }
        }
        scope
    }
}

struct AssertionFacts<'a> {
    span: Span,
    closure_params: HashSet<&'a str>,
    identifiers: Vec<&'a str>,
    properties: Vec<&'a str>,
    uses_receiver: bool,
}

impl<'a> AssertionFacts<'a> {
    fn record(&mut self, expression: &Expr, source: &'a str) {
        let Self {
            closure_params,
            identifiers,
            properties,
            uses_receiver,
            ..
        } = self;
        let name = |identifier: &Identifier| &source[identifier.span.start..identifier.span.end];
        match expression.unparenthesized() {
            Expr::This { .. } | Expr::Super { .. } => *uses_receiver = true,
            Expr::Ident(identifier) => identifiers.push(name(identifier)),
            Expr::Field { field, .. } => properties.push(name(field)),
            Expr::FuncExpr { params, .. } => {
                closure_params.extend(params.params.iter().map(|p| name(&p.name)));
            }
            Expr::Cascade { sections, .. } => {
                for op in sections.iter().flat_map(|section| &section.ops) {
                    match op {
                        CascadeOp::Field { name: field, .. } | CascadeOp::Call(field, ..) => {
                            properties.push(name(field));
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
        }
    }
}

fn needs_instance(
    facts: &AssertionFacts<'_>,
    scope: &ConstructorScope<'_>,
    receiver: &ResolvedType,
    cached_class: Option<&ResolvedType>,
    model: &SemanticModel<'_>,
    signatures: &SignatureIndex,
) -> bool {
    if facts.uses_receiver {
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
    if facts.properties.iter().any(|name| is_cached_member(name)) {
        return true;
    }
    facts.identifiers.iter().any(|name| {
        let name = *name;
        !facts.closure_params.contains(name)
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
    use falcon_analyze::RuleAnalysis;

    #[test]
    fn reports_only_the_assert_keyword() {
        let source = "class Box { Box(int value) { assert /* gap */ (value > 0); assert(value < 10, 'limit'); } }";
        let path = Path::new("/project/lib/test.dart");
        let falcon_dart_parser::ParseResult {
            program,
            errors,
            syntax,
            ..
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
