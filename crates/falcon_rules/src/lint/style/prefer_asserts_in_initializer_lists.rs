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
    AnalyzeContext, DeclarationIdentity, ResolvedType, ResolvedTypeView, Rule, RulePreparation, SemanticModel,
    SignatureIndex, StructuralNode, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct PreferAssertsInInitializerLists;

impl Rule for PreferAssertsInInitializerLists {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let Some(model) = ctx.identities().map(|identities| {
            SemanticModel::new(ctx.file_path(), ctx.parsed(), identities, ctx.types())
        }) else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        let Some(signatures) = ctx.signatures() else {
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
    constructors: HashMap<DeclarationIdentity, HashMap<&'a str, FormalParamList<'a>>>,
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
        params: FormalParamList<'a>,
        mut initializers: AstChildren<'a, ConstructorInitializer<'a>>,
    ) -> ConstructorScope<'a> {
        let super_params = self
            .receiver
            .as_ref()
            .and_then(|receiver| match receiver.view() {
                ResolvedTypeView::Interface { identity, .. } => self.signatures.superclass(identity),
                _ => None,
            })
            .and_then(|superclass| match superclass.view() {
                ResolvedTypeView::Interface { identity, .. } => Some(identity),
                _ => None,
            })
            .and_then(|superclass| {
                let invoked = initializers
                    .find_map(|initializer| match initializer {
                        ConstructorInitializer::SuperCall(call) => {
                            Some(call.call_name().map_or("new", |name| name.name()))
                        }
                        _ => None,
                    })
                    .unwrap_or("new");
                self.constructors.get(superclass)?.get(invoked).copied()
            });
        ConstructorScope::new(params, super_params, self.ctx.source())
    }
}

fn leading_assert_end(body: Option<FunctionBody<'_>>) -> usize {
    let Some(FunctionBody::Block(block)) = body else {
        return 0;
    };
    block
        .stmts()
        .take_while(|statement| matches!(statement, Stmt::Assert(_)))
        .last()
        .map_or(0, |statement| statement.span().end)
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_top_level_decl(&mut self, declaration: TopLevelDecl<'a>) -> StructuralTraversal {
        let name = match declaration {
            TopLevelDecl::Class(class) => class.name().name(),
            TopLevelDecl::MixinClass(class) => class.name().name(),
            TopLevelDecl::Enum(class) => class.name().name(),
            TopLevelDecl::ExtensionType(class) => class.name().name(),
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

    fn visit_constructor(&mut self, constructor: ConstructorRef<'a>) -> StructuralTraversal {
        if constructor.is_primary {
            self.primary_end = leading_assert_end(constructor.body);
            self.primary_scope = (self.primary_end > 0).then(|| {
                self.constructor_scope(constructor.params, constructor.initializers)
            });
        }
        StructuralTraversal::Continue
    }

    fn visit_class_member(&mut self, member: ClassMember<'a>) -> StructuralTraversal {
        match member {
            ClassMember::Constructor(constructor) if !constructor.is_factory() => {
                self.leading_end = leading_assert_end(constructor.body());
                self.scope = (self.leading_end > 0).then(|| {
                    self.constructor_scope(constructor.params(), constructor.initializers())
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

    fn exit_class_member(&mut self, _member: ClassMember<'a>) {
        self.scope = None;
        self.leading_end = 0;
    }

    fn visit_assert_stmt(&mut self, assertion: AssertStmt<'a>) -> StructuralTraversal {
        if self.pending.is_none() && self.scope.is_some() && assertion.span().end <= self.leading_end {
            self.pending = Some(AssertionFacts {
                span: assertion.span(),
                closure_params: HashSet::new(),
                identifiers: Vec::new(),
                properties: Vec::new(),
                uses_receiver: false,
            });
        }
        StructuralTraversal::Continue
    }

    fn visit_expr(&mut self, expression: Expr<'a>) -> StructuralTraversal {
        let Some(facts) = &mut self.pending else {
            return StructuralTraversal::SkipChildren;
        };
        facts.record(expression, self.ctx.source());
        StructuralTraversal::Continue
    }

    fn exit_assert_stmt(&mut self, assertion: AssertStmt<'a>) {
        if self.pending.as_ref().is_none_or(|facts| facts.span != assertion.span()) {
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
                "Assert should be in the initializer list.",
                Span::new(assertion.span().start, assertion.span().start + "assert".len()),
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
    program: Program<'p>,
    model: &SemanticModel<'_>,
) -> HashMap<DeclarationIdentity, HashMap<&'p str, FormalParamList<'p>>> {
    program
        .declarations()
        .filter_map(|declaration| {
            let (name, primary, members) = match declaration {
                TopLevelDecl::Class(node) => {
                    (node.name(), node.primary_constructor(), node.members())
                }
                TopLevelDecl::MixinClass(node) => {
                    (node.name(), node.primary_constructor(), node.members())
                }
                TopLevelDecl::Enum(node) => {
                    (node.name(), node.primary_constructor(), node.members())
                }
                _ => return None,
            };
            let identity = model.resolve_name(name.name())?;
            let primary = primary.map(|primary| {
                (
                    primary.constructor_name().map_or("new", |name| name.name()),
                    primary.params(),
                )
            });
            let constructors = members
                .filter_map(|member| match member {
                    ClassMember::Constructor(constructor) if !constructor.is_factory() => Some((
                        constructor
                            .constructor_name()
                            .map_or("new", |name| name.name()),
                        constructor.params(),
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
        params: FormalParamList<'p>,
        super_params: Option<FormalParamList<'p>>,
        source: &'p str,
    ) -> Self {
        let mut scope = Self {
            params: HashSet::new(),
            initialized_fields: HashSet::new(),
        };
        let mut super_positional = super_params
            .into_iter()
            .flat_map(|params| params.positional());
        for param in params.params() {
            let name = &source[param.name().span().start..param.name().span().end];
            match param.initializing() {
                None => {
                    scope.params.insert(name);
                }
                Some(InitializingFormal::This) => {
                    scope.initialized_fields.insert(name);
                }
                Some(InitializingFormal::Super) => {
                    // The super constructor parameter this formal forwards to:
                    // by position, or by name for a named formal.
                    let named = matches!(param.kind(), ParamKind::Named { .. });
                    let target = if named {
                        super_params.and_then(|params| {
                            params
                                .named()
                                .find(|candidate| candidate.name().name() == name)
                        })
                    } else {
                        super_positional.next()
                    };
                    if let Some(target) = target
                        && target.initializing() == Some(InitializingFormal::This)
                    {
                        scope
                            .initialized_fields
                            .insert(&source[target.name().span().start..target.name().span().end]);
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
    fn record(&mut self, expression: Expr<'a>, source: &'a str) {
        let Self {
            closure_params,
            identifiers,
            properties,
            uses_receiver,
            ..
        } = self;
        let name =
            |identifier: Identifier<'a>| &source[identifier.span().start..identifier.span().end];
        match expression.unparenthesized() {
            Expr::This(_) | Expr::Super(_) => *uses_receiver = true,
            Expr::Ident(identifier) => identifiers.push(name(identifier)),
            Expr::Field(field) => properties.push(name(field.field())),
            Expr::FuncExpr(function) => {
                closure_params.extend(function.params().params().map(|param| name(param.name())));
            }
            Expr::Cascade(cascade) => {
                for op in cascade.sections().flat_map(|section| section.ops()) {
                    match op {
                        CascadeOp::Field(field) => properties.push(name(field.name())),
                        CascadeOp::Call(call) => properties.push(name(call.name())),
                        CascadeOp::Invoke(_)
                        | CascadeOp::NullAssert(_)
                        | CascadeOp::Index(_)
                        | CascadeOp::Assign(_)
                        | CascadeOp::IndexAssign(_) => {}
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
            let ResolvedTypeView::Interface { identity, .. } = receiver.view() else {
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
    use std::{path::Path, sync::Arc};

    use falcon_analyze::{ParsedFile};
    use falcon_config::FalconConfig;
    use falcon_dart_parser::parse;

    use super::*;
    use falcon_analyze::RuleAnalysis;

    #[test]
    fn reports_only_the_assert_keyword() {
        let source = "class Box { Box(int value) { assert /* gap */ (value > 0); assert(value < 10, 'limit'); } }";
        let path = Path::new("/project/lib/test.dart");
        let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
        assert!(errors.is_empty(), "{errors:?}");
        let parsed = Arc::new(ParsedFile {
            source: source.into(),
            tree,
        });
        let program = parsed.program();
        let indexes = crate::lint::test_support::TestIndexes::single(path, &parsed, false);
        let config = FalconConfig::default();
        let ctx = indexes.context(path, &config, &parsed);
        let diagnostics = PreferAssertsInInitializerLists.analyze(PreferAssertsInInitializerLists::NAME, program, &ctx);
        let slices: Vec<_> = diagnostics
            .iter()
            .map(|diagnostic| &source[diagnostic.span.start..diagnostic.span.end])
            .collect();
        assert_eq!(slices, ["assert", "assert"]);
    }
}
