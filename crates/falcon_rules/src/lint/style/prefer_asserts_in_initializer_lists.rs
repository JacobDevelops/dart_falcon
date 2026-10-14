//! Flags leading constructor-body asserts that could live in the initializer list.
//!
//! An `assert` at the start of a generative constructor body that does not
//! need the instance (no `this`, no instance member of the class or its
//! superclasses and mixins) can move into the initializer list, where it runs
//! before the object is used and works for `const` constructors too. Fields
//! initialized by `this.x` formals count as available, since the formal holds
//! the same value.
//!
//! ```dart
//! class A {
//!   A(int a) { assert(a != 0); } // flagged
//!   A(int a) : assert(a != 0);   // ok
//! }
//! ```
//!
//! Members inherited from classes declared outside this file are unknown, so a
//! bare identifier that could resolve to one keeps the rule silent, as does a
//! field initialized through a `super.x` formal (falcon does not trace it to
//! the superclass's `this.x` parameter).
//!
//! Adopted from the official `prefer_asserts_in_initializer_lists` lint.

use std::collections::{HashMap, HashSet};

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::for_each_expr;

#[derive(Default)]
pub struct PreferAssertsInInitializerLists;

impl Rule for PreferAssertsInInitializerLists {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let types = TypeMembers::collect(program);
        let mut diags = Vec::new();
        for declaration in &program.declarations {
            let (name, members) = match declaration {
                TopLevelDecl::Class(class) => (&class.name.name, &class.members),
                TopLevelDecl::MixinClass(class) => (&class.name.name, &class.members),
                _ => continue,
            };
            let chain = types.chain(name);
            let own_fields = &types.by_name[name.as_str()].fields;
            for member in members {
                let ClassMember::Constructor(constructor) = member else {
                    continue;
                };
                if constructor.is_factory {
                    continue;
                }
                let Some(FunctionBody::Block(block)) = &constructor.body else {
                    continue;
                };
                let scope = ConstructorScope::new(&constructor.params, own_fields);
                for statement in &block.stmts {
                    let Stmt::Assert(assert) = statement else {
                        break;
                    };
                    if !needs_instance(assert, &scope, chain.as_ref()) {
                        diags.push(ctx.diagnostic(
                            "prefer-asserts-in-initializer-lists",
                            "Assert should be in the initializer list.",
                            assert.span,
                        ));
                    }
                }
            }
        }
        diags
    }
}

#[derive(Default)]
struct Members<'p> {
    instance: HashSet<&'p str>,
    fields: HashSet<&'p str>,
    supertypes: Vec<Option<&'p str>>,
}

impl<'p> Members<'p> {
    fn add(&mut self, name: &'p Identifier, is_static: bool, is_field: bool) {
        // Static members never need the instance.
        if is_static {
            return;
        }
        let name = name.name.as_str();
        self.instance.insert(name);
        if is_field {
            self.fields.insert(name);
        }
    }
}

struct TypeMembers<'p> {
    by_name: HashMap<&'p str, Members<'p>>,
}

/// Instance member names of a class and its superclass/mixin chain.
struct Chain<'p> {
    instance: HashSet<&'p str>,
}

fn simple_name(ty: &DartType) -> Option<&str> {
    match ty {
        DartType::Named(named) => match named.segments.as_slice() {
            [segment] => Some(segment.name.as_str()),
            _ => None,
        },
        _ => None,
    }
}

impl<'p> TypeMembers<'p> {
    fn collect(program: &'p Program) -> Self {
        let mut by_name = HashMap::new();
        for declaration in &program.declarations {
            let (name, extends, with_clause, members): (_, _, &[DartType], _) = match declaration {
                TopLevelDecl::Class(class) => (
                    &class.name,
                    class.extends.as_ref(),
                    &class.with_clause,
                    &class.members,
                ),
                TopLevelDecl::MixinClass(class) => (
                    &class.name,
                    class.extends.as_ref(),
                    &class.with_clause,
                    &class.members,
                ),
                TopLevelDecl::Mixin(mixin) => (&mixin.name, None, &[], &mixin.members),
                _ => continue,
            };
            let mut entry = Members {
                supertypes: extends
                    .into_iter()
                    .chain(with_clause)
                    .map(simple_name)
                    .collect(),
                ..Members::default()
            };
            for member in members {
                match member {
                    ClassMember::Field(field) => {
                        for declarator in &field.declarators {
                            entry.add(&declarator.name, field.is_static, true);
                        }
                    }
                    ClassMember::Method(m) => entry.add(&m.name, m.is_static, false),
                    ClassMember::Getter(g) => entry.add(&g.name, g.is_static, false),
                    ClassMember::Setter(s) => entry.add(&s.name, s.is_static, false),
                    ClassMember::Constructor(_)
                    | ClassMember::Operator(_)
                    | ClassMember::Error(_) => {}
                }
            }
            by_name.insert(name.name.as_str(), entry);
        }
        Self { by_name }
    }

    /// `None` when some superclass or mixin is declared outside this file.
    fn chain(&self, name: &str) -> Option<Chain<'p>> {
        let mut chain = Chain {
            instance: HashSet::new(),
        };
        let mut visited = HashSet::new();
        let mut pending = vec![Some(name)];
        while let Some(next) = pending.pop() {
            let next = next?;
            if next == "Object" || !visited.insert(next) {
                continue;
            }
            let members = self.by_name.get(next)?;
            chain.instance.extend(&members.instance);
            pending.extend(members.supertypes.iter().copied());
        }
        Some(chain)
    }
}

/// Names a constructor body sees without needing the instance.
struct ConstructorScope<'p> {
    /// Regular (non-initializing) formals, which shadow members.
    params: HashSet<&'p str>,
    /// Own fields initialized by `this.x` formals.
    initialized_fields: HashSet<&'p str>,
}

impl<'p> ConstructorScope<'p> {
    fn new(params: &'p FormalParamList, own_fields: &HashSet<&'p str>) -> Self {
        let mut scope = Self {
            params: HashSet::new(),
            initialized_fields: HashSet::new(),
        };
        for param in &params.params {
            let name = param.name.name.as_str();
            match param.initializing {
                None => {
                    scope.params.insert(name);
                }
                Some(InitializingFormal::This) if own_fields.contains(name) => {
                    scope.initialized_fields.insert(name);
                }
                Some(InitializingFormal::This | InitializingFormal::Super) => {}
            }
        }
        scope
    }
}

fn needs_instance(
    assert: &AssertStmt,
    scope: &ConstructorScope<'_>,
    chain: Option<&Chain<'_>>,
) -> bool {
    let mut closure_params = HashSet::new();
    let mut identifiers = Vec::new();
    let mut properties = Vec::new();
    let mut uses_receiver = false;
    let mut collect = |expression: &Expr| match expression {
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
                    CascadeOp::Index { .. } | CascadeOp::Assign(..) => {}
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
    let Some(chain) = chain else {
        // An unknown superclass may declare any free identifier as a member.
        return identifiers
            .iter()
            .any(|name| !closure_params.contains(name) && !scope.params.contains(name.as_str()));
    };
    // A selector may resolve to a member of this class on another receiver.
    if properties
        .iter()
        .any(|name| chain.instance.contains(name.as_str()))
    {
        return true;
    }
    identifiers.iter().any(|name| {
        let name = name.as_str();
        !closure_params.contains(name)
            && !scope.params.contains(name)
            && chain.instance.contains(name)
            && !scope.initialized_fields.contains(name)
    })
}
