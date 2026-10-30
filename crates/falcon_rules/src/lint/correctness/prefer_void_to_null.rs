//! Flags uses of the `Null` type where `void` is meant.
//!
//! `Null` and `void` both describe "no useful value", but `void` says the value
//! must not be used while `Null` admits only `null` and is a subtype of every
//! nullable type. Writing `Future<Null>` or a `Null` return type usually means
//! `void`, and only `void` lets implementations return whatever they like.
//!
//! ```dart
//! Null f() {}                   // flagged
//! Future<Null> g() async {}     // flagged
//! void h() {}                   // preferred
//! ```
//!
//! Following the official lint, `Null` is accepted where `void` cannot replace
//! it: casts (`as Null`, cast patterns), local and top-level variable types,
//! the return and parameter types written directly in a function type, type
//! arguments of empty collection literals (`<Null>[]`), `extension on Null`,
//! extension type representations, and return types of members overriding a
//! member that does not return `void` after substituting inherited type arguments.
//! Overrides whose external declarations are unavailable are conservatively
//! exempt; an unresolved source superclass is not evidence of an override.
//!
//! Adopted from the official `prefer_void_to_null` lint.

use std::collections::HashSet;

use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, MemberKind, QualifiedName, ResolvedType, ResolvedTypeView, Rule,
    RulePreparation, SemanticModel, SignatureIndex, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct PreferVoidToNull;

impl Rule for PreferVoidToNull {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let (Some(identities), Some(signatures)) = (ctx.identities(), ctx.signatures()) else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        let model = SemanticModel::new(ctx.file_path(), ctx.parsed(), identities, ctx.types());
        let finder = NullTypes {
            model,
            signatures,
            enclosing: None,
            exempt: HashSet::new(),
            spans: Vec::new(),
            ctx,
            enclosing_stack: Vec::new(),
        };
        RulePreparation::Structural(Box::new(finder))
    }
}

/// The interface-like declaration whose members are being visited.
struct Enclosing {
    owner: Option<DeclarationIdentity>,
    supertypes: Vec<ResolvedType>,
}

struct NullTypes<'a> {
    model: SemanticModel<'a>,
    signatures: &'a SignatureIndex,
    enclosing: Option<Enclosing>,
    /// Span starts of type annotations whose own `Null` is accepted.
    exempt: HashSet<usize>,
    spans: Vec<Span>,
    ctx: &'a AnalyzeContext<'a>,
    enclosing_stack: Vec<Option<Enclosing>>,
}

impl<'a> NullTypes<'a> {
    /// The `Null` name token, if `ty` is the `dart:core` `Null` type.
    fn null_name(&self, ty: DartType<'a>) -> Option<Span> {
        let DartType::Named(named) = ty else {
            return None;
        };
        let last = named.segments().last()?;
        if last.name() != "Null" {
            return None;
        }
        let identity = self
            .model
            .resolve_name(QualifiedName::from_identifiers(named.segments())?)?;
        identity.is_sdk("dart:core", "Null").then_some(last.span())
    }

    fn exempt(&mut self, ty: Option<DartType<'a>>) {
        if let Some(ty) = ty {
            self.exempt.insert(ty.span().start);
        }
    }

    /// Whether a `Null` return type of member `name` overrides a member that
    /// does not return `void` (or is unresolvable).
    fn incompatible_override(&self, name: &str, getter: bool) -> bool {
        let Some(enclosing) = &self.enclosing else {
            // Extension members override nothing.
            return false;
        };
        let Some(owner) = &enclosing.owner else {
            return true;
        };
        let Some(inherited) = self.signatures.inherited_member_facts(owner, name) else {
            return !enclosing
                .supertypes
                .iter()
                .any(|ty| matches!(ty.view(), ResolvedTypeView::Unknown));
        };
        let overrides = inherited.iter().any(|fact| {
            if getter {
                fact.kind.has_getter()
            } else {
                fact.kind == MemberKind::Method
            }
        });
        if !overrides {
            return false;
        }
        let mut overrides_void = false;
        for supertype in &enclosing.supertypes {
            let returned =
                self.signatures
                    .instantiated_override_return(supertype, name, getter, &self.model);
            match returned {
                Some(ty) if is_void_like(&ty) => overrides_void = true,
                Some(_) => return true,
                None => {}
            }
        }
        !overrides_void
    }
}

fn resolve_all<'t>(
    model: &SemanticModel<'_>,
    types: impl Iterator<Item = DartType<'t>>,
) -> Vec<ResolvedType> {
    types.map(|ty| model.resolve_type(ty)).collect()
}

/// `void` or `FutureOr<void>`.
fn is_void_like(ty: &ResolvedType) -> bool {
    match ty.view() {
        ResolvedTypeView::Void => true,
        ResolvedTypeView::Interface {
            identity,
            arguments,
            ..
        } => {
            identity.is_sdk("dart:async", "FutureOr")
                && arguments.len() == 1
                && matches!(arguments[0].view(), ResolvedTypeView::Void)
        }
        _ => false,
    }
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for NullTypes<'a> {
    fn finish(self: Box<Self>) -> Vec<falcon_diagnostics::Diagnostic> {
        self.spans.into_iter().map(|span| self.ctx.diagnostic(
            "Avoid using the type `Null`.",
            span,
        )).collect()
    }

    fn visit_top_level_decl(&mut self, node: TopLevelDecl<'a>) -> StructuralTraversal {
        self.enclosing_stack.push(self.enclosing.take());
        let model = self.model;
        let owner = |name: Identifier<'a>| model.resolve_name(name.name());
        match node {
            TopLevelDecl::Variable(variable) => {
                self.exempt(variable.var_type());
            }
            TopLevelDecl::Extension(extension) => {
                self.exempt(Some(extension.on_type()));
                self.enclosing = None;
            }
            TopLevelDecl::ExtensionType(extension_type) => {
                self.exempt(
                    extension_type
                        .representation()
                        .and_then(falcon_syntax::cst::FormalParam::param_type),
                );
                let enclosing = Enclosing {
                    owner: owner(extension_type.name()),
                    supertypes: resolve_all(&model, extension_type.implements()),
                };
                self.enclosing = Some(enclosing);
            }
            TopLevelDecl::Class(class) => {
                let enclosing = Enclosing {
                    owner: owner(class.name()),
                    supertypes: resolve_all(
                        &model,
                        class
                            .extends()
                            .into_iter()
                            .chain(class.with_clause())
                            .chain(class.implements()),
                    ),
                };
                self.enclosing = Some(enclosing);
            }
            TopLevelDecl::MixinClass(class) => {
                let enclosing = Enclosing {
                    owner: owner(class.name()),
                    supertypes: resolve_all(
                        &model,
                        class
                            .extends()
                            .into_iter()
                            .chain(class.with_clause())
                            .chain(class.implements()),
                    ),
                };
                self.enclosing = Some(enclosing);
            }
            TopLevelDecl::Mixin(mixin) => {
                let enclosing = Enclosing {
                    owner: owner(mixin.name()),
                    supertypes: resolve_all(&model, mixin.on_clause().chain(mixin.implements())),
                };
                self.enclosing = Some(enclosing);
            }
            TopLevelDecl::Enum(enumeration) => {
                let enclosing = Enclosing {
                    owner: owner(enumeration.name()),
                    supertypes: resolve_all(
                        &model,
                        enumeration
                            .with_clause()
                            .chain(enumeration.implements()),
                    ),
                };
                self.enclosing = Some(enclosing);
            }
            TopLevelDecl::ClassTypeAlias(_)
            | TopLevelDecl::Function(_)
            | TopLevelDecl::TypeAlias(_)
            | TopLevelDecl::Error(_) => {},
        }
        StructuralTraversal::Continue
    }

    fn exit_top_level_decl(&mut self, _node: TopLevelDecl<'a>) {
        self.enclosing = self.enclosing_stack.pop().unwrap();
    }

    fn visit_method_decl(&mut self, node: MethodDecl<'a>) -> StructuralTraversal {
        if let Some(return_type) = node.return_type()
            && self.null_name(return_type).is_some()
            && self.incompatible_override(node.name().name(), false)
        {
            self.exempt(Some(return_type));
        }
        StructuralTraversal::Continue
    }

    fn visit_getter_decl(&mut self, node: GetterDecl<'a>) -> StructuralTraversal {
        if let Some(return_type) = node.return_type()
            && self.null_name(return_type).is_some()
            && self.incompatible_override(node.name().name(), true)
        {
            self.exempt(Some(return_type));
        }
        StructuralTraversal::Continue
    }

    fn visit_local_var(&mut self, node: LocalVarDecl<'a>) -> StructuralTraversal {
        self.exempt(node.var_type());
        StructuralTraversal::Continue
    }

    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        match node {
            Expr::As(expression) => self.exempt(Some(expression.dart_type())),
            Expr::List(expression) if expression.elements().is_empty() => {
                self.exempt(expression.type_arg());
            }
            Expr::Set(expression) if expression.elements().is_empty() => {
                self.exempt(expression.type_arg());
            }
            Expr::Map(expression) if expression.elements().is_empty() => {
                for type_arg in expression.type_args() {
                    self.exempt(Some(type_arg));
                }
            }
            _ => {}
        }
        StructuralTraversal::Continue
    }

    fn visit_pattern(&mut self, node: Pattern<'a>) -> StructuralTraversal {
        if let Pattern::Cast(pattern) = node {
            self.exempt(Some(pattern.cast_type()));
        }
        StructuralTraversal::Continue
    }

    fn visit_dart_type(&mut self, node: DartType<'a>) -> StructuralTraversal {
        if let Some(span) = self.null_name(node)
            && !self.exempt.contains(&node.span().start)
        {
            self.spans.push(span);
        }
        if let DartType::Function(function) = node {
            if let Some(return_type) = function.return_type() {
                self.exempt(Some(return_type));
            }
            for param in function.params() {
                self.exempt(Some(param.param_type()));
            }
        }
        StructuralTraversal::Continue
    }
}
}
