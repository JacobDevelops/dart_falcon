//! Flags arguments that pass exactly the parameter's default value.
//!
//! An optional parameter's default already applies when the argument is
//! omitted, so `f(flag: false)` against `void f({bool flag = false})` is noise.
//! The rule compares each constant argument with the constant default of the
//! optional parameter it binds to and reports matches. Arguments are visited
//! from last to first, stopping after the first optional positional parameter:
//! an earlier positional argument may be needed to reach a later one. For a
//! redirecting factory constructor the defaults of the final redirect target
//! apply. Required parameters and `@required` ones are never reported.
//!
//! falcon reads defaults from source, so only callees declared in the analyzed
//! file are checked (top-level functions, methods, constructors, enum constants
//! and annotations); calls into the SDK, packages or other files are skipped, as
//! are values its constant evaluator cannot fold.
//!
//! Adopted from the official `avoid_redundant_argument_values` lint.

use std::collections::{HashMap, HashSet};

use falcon_analyze::{
    AnalyzeContext, ConstantValue, DeclarationIdentity, ResolvedType, Rule, evaluate_constant,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr};

use crate::lint::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};
use crate::lint::style::use_named_constants::ConstContexts;

#[derive(Default)]
pub struct AvoidRedundantArgumentValues;

impl Rule for AvoidRedundantArgumentValues {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(state) = SemanticState::new(program, ctx) else {
            return Vec::new();
        };
        let declarations = Declarations::new(program);
        let mut collector = Collector {
            diags: Vec::new(),
            ctx,
            declarations: &declarations,
            contexts: ConstContexts::new(program),
        };
        let state = visit_program(&mut collector, program, state);
        collector.visit_declarations(program, &state);
        collector.diags
    }
}

/// A type declared in this file with the members whose defaults matter.
#[derive(Default)]
struct TypeDecl<'p> {
    constructors: HashMap<&'p str, &'p ConstructorDecl>,
    methods: HashMap<&'p str, &'p FormalParamList>,
    static_consts: HashMap<&'p str, &'p Expr>,
    field_types: HashMap<&'p str, &'p DartType>,
}

struct Declarations<'p> {
    functions: HashMap<&'p str, &'p FormalParamList>,
    types: HashMap<&'p str, TypeDecl<'p>>,
}

impl<'p> Declarations<'p> {
    fn new(program: &'p Program) -> Self {
        let mut functions = HashMap::new();
        let mut types = HashMap::new();
        for declaration in &program.declarations {
            let (name, members) = match declaration {
                TopLevelDecl::Function(function) => {
                    if let FunctionKind::Function(params) = &function.kind {
                        functions.insert(function.name.name.as_str(), params);
                    }
                    continue;
                }
                TopLevelDecl::Class(class) => (&class.name, &class.members),
                TopLevelDecl::Mixin(mixin) => (&mixin.name, &mixin.members),
                TopLevelDecl::MixinClass(class) => (&class.name, &class.members),
                TopLevelDecl::Enum(declaration) => (&declaration.name, &declaration.members),
                TopLevelDecl::ExtensionType(extension) => (&extension.name, &extension.members),
                _ => continue,
            };
            let mut type_decl = TypeDecl::default();
            for member in members {
                match member {
                    ClassMember::Constructor(constructor) => {
                        let name = constructor
                            .constructor_name
                            .as_ref()
                            .map_or("new", |name| name.name.as_str());
                        type_decl.constructors.insert(name, constructor);
                    }
                    ClassMember::Method(method) => {
                        type_decl
                            .methods
                            .insert(method.name.name.as_str(), &method.params);
                    }
                    ClassMember::Field(field) => {
                        let static_const = field.is_static && field.keyword == VarKeyword::Const;
                        for declarator in &field.declarators {
                            let name = declarator.name.name.as_str();
                            if let Some(ty) = &field.field_type {
                                type_decl.field_types.insert(name, ty);
                            }
                            if static_const && let Some(initializer) = &declarator.initializer {
                                type_decl.static_consts.insert(name, initializer);
                            }
                        }
                    }
                    _ => {}
                }
            }
            types.insert(name.name.as_str(), type_decl);
        }
        Self { functions, types }
    }
}

/// The parameters an invocation binds, plus the declaration owning them.
struct Target<'p> {
    params: &'p FormalParamList,
    owner: Option<(DeclarationIdentity, &'p TypeDecl<'p>)>,
}

struct Collector<'a, 'p> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
    declarations: &'p Declarations<'p>,
    contexts: ConstContexts,
}

impl<'p> Collector<'_, 'p> {
    /// The in-file type named by `identity`, when `identity` is the local one.
    fn local_type(
        &self,
        identity: &DeclarationIdentity,
        state: &SemanticState<'_>,
    ) -> Option<&'p TypeDecl<'p>> {
        let DeclarationIdentity::Project { name, .. } = identity else {
            return None;
        };
        let declared = self.declarations.types.get(name.as_str())?;
        (state.model.resolve_name(name.as_str()).as_ref() == Some(identity)).then_some(declared)
    }

    fn local_type_named(
        &self,
        name: &str,
        state: &SemanticState<'_>,
    ) -> Option<(DeclarationIdentity, &'p TypeDecl<'p>)> {
        let identity = state.model.resolve_name(name)?;
        let declared = self.local_type(&identity, state)?;
        Some((identity, declared))
    }

    /// Resolves a constructor, following redirecting factories to their target.
    fn constructor_target(
        &self,
        identity: DeclarationIdentity,
        name: &str,
        state: &SemanticState<'_>,
    ) -> Option<Target<'p>> {
        let mut identity = identity;
        let mut name = name.to_string();
        let mut visited = HashSet::new();
        loop {
            let declared = self.local_type(&identity, state)?;
            let constructor = *declared.constructors.get(name.as_str())?;
            let Some(redirect) = &constructor.redirect else {
                return Some(Target {
                    params: &constructor.params,
                    owner: Some((identity, declared)),
                });
            };
            if !visited.insert((identity.clone(), name.clone())) {
                return None;
            }
            let ResolvedType::Interface {
                identity: target, ..
            } = state.model.resolve_type(&redirect.type_)
            else {
                return None;
            };
            identity = target;
            name = redirect
                .constructor_name
                .as_ref()
                .map_or_else(|| "new".to_string(), |name| name.name.clone());
        }
    }

    fn method_target(
        &self,
        identity: DeclarationIdentity,
        name: &str,
        state: &SemanticState<'_>,
    ) -> Option<Target<'p>> {
        let declared = self.local_type(&identity, state)?;
        let params = *declared.methods.get(name)?;
        Some(Target {
            params,
            owner: Some((identity, declared)),
        })
    }

    fn enclosing_type(&self, state: &SemanticState<'_>) -> Option<DeclarationIdentity> {
        match state.enclosing_type.as_ref()? {
            ResolvedType::Interface { identity, .. } => Some(identity.clone()),
            _ => None,
        }
    }

    fn call_target(&self, callee: &Expr, state: &SemanticState<'_>) -> Option<Target<'p>> {
        match callee {
            Expr::Ident(identifier) => {
                let name = identifier.name.as_str();
                if state.environment.is_bound(name) {
                    return None;
                }
                // Lexical scope: members declared in the enclosing type, then the
                // library, then inherited instance members via implicit `this`.
                let enclosing = self.enclosing_type(state);
                if let Some(identity) = &enclosing
                    && let Some(declared) = self.local_type(identity, state)
                    && declared.methods.contains_key(name)
                {
                    return self.method_target(identity.clone(), name, state);
                }
                if let Some(params) = self.declarations.functions.get(name) {
                    return state.model.resolve_value(name).is_some().then_some(Target {
                        params,
                        owner: None,
                    });
                }
                if let Some((identity, _)) = self.local_type_named(name, state) {
                    return self.constructor_target(identity, "new", state);
                }
                let receiver = state.enclosing_type.as_ref()?;
                let owner = state
                    .signatures
                    .resolved_member_owner(receiver, name, &state.model)?;
                self.method_target(owner, name, state)
            }
            Expr::Field { object, field, .. } => {
                if let Expr::Ident(prefix) = object.as_ref()
                    && !state.environment.is_bound(&prefix.name)
                    && let Some((identity, declared)) = self.local_type_named(&prefix.name, state)
                {
                    let name = field.name.as_str();
                    return if declared.constructors.contains_key(name) {
                        self.constructor_target(identity, name, state)
                    } else {
                        self.method_target(identity, name, state)
                    };
                }
                if matches!(object.as_ref(), Expr::Super { .. }) {
                    return None;
                }
                let receiver = state.infer(object);
                let owner =
                    state
                        .signatures
                        .resolved_member_owner(&receiver, &field.name, &state.model)?;
                self.method_target(owner, &field.name, state)
            }
            _ => None,
        }
    }

    fn check(&mut self, args: &ArgList, target: &Target<'_>, state: &SemanticState<'_>) {
        let positional: Vec<&FormalParam> = target.params.positional().collect();
        let mut arguments: Vec<(&Expr, Option<&FormalParam>)> = args
            .positional
            .iter()
            .enumerate()
            .map(|(index, argument)| (argument, positional.get(index).copied()))
            .chain(args.named.iter().map(|argument| {
                (
                    &argument.value,
                    target
                        .params
                        .named()
                        .find(|param| param.name.name == argument.name.name),
                )
            }))
            .collect();
        arguments.sort_by_key(|(argument, _)| argument.span().start);
        let caller = self.argument_owner(state);
        for (argument, param) in arguments.into_iter().rev() {
            let Some(param) = param else {
                continue;
            };
            self.check_argument(argument, param, target, caller.as_ref(), state);
            if param.kind == ParamKind::OptionalPositional {
                break;
            }
        }
    }

    /// The in-file type whose static constants the call site can name unqualified.
    fn argument_owner(
        &self,
        state: &SemanticState<'_>,
    ) -> Option<(DeclarationIdentity, &'p TypeDecl<'p>)> {
        let identity = self.enclosing_type(state)?;
        let declared = self.local_type(&identity, state)?;
        Some((identity, declared))
    }

    fn check_argument(
        &mut self,
        argument: &Expr,
        param: &FormalParam,
        target: &Target<'_>,
        caller: Option<&(DeclarationIdentity, &TypeDecl<'_>)>,
        state: &SemanticState<'_>,
    ) {
        let optional = matches!(
            param.kind,
            ParamKind::OptionalPositional | ParamKind::Named { required: false }
        );
        let annotated_required = param.annotations.iter().any(|annotation| {
            annotation
                .name
                .last()
                .is_some_and(|name| name.name == "required")
        });
        let Some(default) = &param.default_value else {
            return;
        };
        if !optional || annotated_required || !self.is_constant(argument) {
            return;
        }
        let empty = HashMap::new();
        let (owner, fields) = target
            .owner
            .as_ref()
            .map_or((None, &empty), |(identity, declared)| {
                (Some(identity), &declared.static_consts)
            });
        let Some(default_value) =
            evaluate_constant(default, owner, fields, &state.model, &state.signatures)
        else {
            return;
        };
        let (caller_owner, caller_fields) = caller
            .map_or((None, &empty), |(identity, declared)| {
                (Some(identity), &declared.static_consts)
            });
        let Some(value) = evaluate_constant(
            argument,
            caller_owner,
            caller_fields,
            &state.model,
            &state.signatures,
        ) else {
            return;
        };
        let declared_type = param.param_type.as_ref().or_else(|| {
            let (_, declared) = target.owner.as_ref()?;
            (param.initializing == Some(InitializingFormal::This))
                .then(|| declared.field_types.get(param.name.name.as_str()).copied())
                .flatten()
        });
        if value != default_value || !same_numeric_kind(&value, argument, default, declared_type) {
            return;
        }
        self.diags.push(self.ctx.diagnostic(
            "avoid-redundant-argument-values",
            "The value of the argument is redundant because it matches the default value.",
            *argument.span(),
        ));
    }

    /// Whether `argument` is a constant expression: collection literals and
    /// constructor calls only count when they are in a constant context.
    fn is_constant(&self, argument: &Expr) -> bool {
        if self.contexts.contains(argument.span()) {
            return true;
        }
        let mut checker = NonConstantFinder { found: false };
        checker.visit_expr(argument);
        !checker.found
    }

    fn visit_declarations(&mut self, program: &Program, state: &SemanticState<'_>) {
        for declaration in &program.declarations {
            match declaration {
                TopLevelDecl::Enum(declaration) => {
                    let Some((identity, _)) = self.local_type_named(&declaration.name.name, state)
                    else {
                        continue;
                    };
                    for variant in &declaration.variants {
                        let Some(args) = &variant.args else {
                            continue;
                        };
                        let name = variant
                            .constructor_name
                            .as_ref()
                            .map_or("new", |name| name.name.as_str());
                        if let Some(target) = self.constructor_target(identity.clone(), name, state)
                        {
                            self.check(args, &target, state);
                        }
                    }
                }
                _ => continue,
            }
        }
        let mut annotations = AnnotationCollector::default();
        annotations.visit_program(program);
        for annotation in annotations.found {
            let (Some(args), [name]) = (&annotation.args, annotation.name.as_slice()) else {
                continue;
            };
            let Some((identity, _)) = self.local_type_named(&name.name, state) else {
                continue;
            };
            let constructor = annotation
                .constructor_name
                .as_ref()
                .map_or("new", |name| name.name.as_str());
            if let Some(target) = self.constructor_target(identity, constructor, state) {
                self.check(args, &target, state);
            }
        }
    }
}

/// Numbers fold to one value regardless of `int`/`double`; Dart constants keep
/// the distinction unless the parameter is a `double` (where an integer literal
/// denotes a double).
fn same_numeric_kind(
    value: &ConstantValue,
    argument: &Expr,
    default: &Expr,
    declared_type: Option<&DartType>,
) -> bool {
    if !matches!(value, ConstantValue::Number(_)) {
        return true;
    }
    let double_param = matches!(declared_type, Some(DartType::Named(ty))
        if !ty.is_nullable && ty.segments.len() == 1 && ty.segments[0].name == "double");
    match (
        numeric_literal_kind(argument),
        numeric_literal_kind(default),
    ) {
        (Some(left), Some(right)) => left == right || double_param,
        // A named constant hides its literal; trust the folded value.
        _ => true,
    }
}

/// `Some(true)` for a (negated) double literal, `Some(false)` for an int one.
fn numeric_literal_kind(expr: &Expr) -> Option<bool> {
    let mut current = expr;
    while let Expr::Unary {
        op: UnaryOp::Minus,
        operand,
        ..
    } = current
    {
        current = operand;
    }
    match current {
        Expr::IntLit { .. } => Some(false),
        Expr::DoubleLit { .. } => Some(true),
        _ => None,
    }
}

/// Finds a sub-expression that is not constant outside a constant context.
struct NonConstantFinder {
    found: bool,
}

impl Visitor for NonConstantFinder {
    fn visit_expr(&mut self, node: &Expr) {
        match node {
            Expr::List { is_const, .. }
            | Expr::Map { is_const, .. }
            | Expr::Set { is_const, .. }
            | Expr::Record { is_const, .. }
            | Expr::New { is_const, .. } => {
                // An explicit `const` makes its whole subtree constant.
                if !is_const {
                    self.found = true;
                }
            }
            Expr::Call { .. } | Expr::FuncExpr { .. } | Expr::This { .. } => self.found = true,
            _ => walk_expr(self, node),
        }
    }
}

#[derive(Default)]
struct AnnotationCollector {
    found: Vec<Annotation>,
}

impl Visitor for AnnotationCollector {
    fn visit_annotation(&mut self, node: &Annotation) {
        if node.args.is_some() {
            self.found.push(node.clone());
        }
    }
}

impl SemanticRuleVisitor for Collector<'_, '_> {
    fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
        let target = match node {
            Expr::Call { callee, args, .. } => self.call_target(callee, state).map(|t| (t, args)),
            Expr::New {
                dart_type,
                constructor_name,
                args,
                ..
            } => match state.model.resolve_type(dart_type) {
                ResolvedType::Interface { identity, .. } => self
                    .constructor_target(
                        identity,
                        constructor_name
                            .as_ref()
                            .map_or("new", |name| name.name.as_str()),
                        state,
                    )
                    .map(|t| (t, args)),
                _ => None,
            },
            _ => None,
        };
        if let Some((target, args)) = target {
            self.check(args, &target, state);
        }
    }
}
