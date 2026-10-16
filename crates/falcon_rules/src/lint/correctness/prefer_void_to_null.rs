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
    AnalyzeContext, DeclarationIdentity, MemberKind, QualifiedName, ResolvedType, Rule,
    RulePreparation, SemanticModel, SignatureIndex,
};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{
    Visitor, walk_dart_type, walk_expr, walk_getter_decl, walk_local_var, walk_method_decl,
    walk_pattern, walk_top_level_decl,
};

#[derive(Default)]
pub struct PreferVoidToNull;

impl Rule for PreferVoidToNull {
    fn prepare<'a>(
        &self,
        program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let (Some(identities), Some(signatures)) = (ctx.identities, ctx.signatures) else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        let model = SemanticModel::new(ctx.file_path, identities, ctx.types);
        let mut finder = NullTypes {
            model: &model,
            signatures,
            enclosing: None,
            exempt: HashSet::new(),
            spans: Vec::new(),
        };
        finder.visit_program(program);
        RulePreparation::Diagnostics(
            finder
                .spans
                .into_iter()
                .map(|span| {
                    ctx.diagnostic(
                        "prefer-void-to-null",
                        "Unnecessary use of the type 'Null'.",
                        span,
                    )
                })
                .collect(),
        )
    }
}

/// The interface-like declaration whose members are being visited.
struct Enclosing {
    owner: Option<DeclarationIdentity>,
    supertypes: Vec<ResolvedType>,
}

struct NullTypes<'a> {
    model: &'a SemanticModel<'a>,
    signatures: &'a SignatureIndex,
    enclosing: Option<Enclosing>,
    /// Span starts of type annotations whose own `Null` is accepted.
    exempt: HashSet<usize>,
    spans: Vec<Span>,
}

impl<'a> NullTypes<'a> {
    /// The `Null` name token, if `ty` is the `dart:core` `Null` type.
    fn null_name(&self, ty: &DartType) -> Option<Span> {
        let DartType::Named(named) = ty else {
            return None;
        };
        let last = named.segments.last()?;
        if last.name != "Null" {
            return None;
        }
        let identity = self
            .model
            .resolve_name(QualifiedName::from_identifiers(&named.segments)?)?;
        identity.is_sdk("dart:core", "Null").then_some(last.span)
    }

    fn exempt(&mut self, ty: Option<&DartType>) {
        if let Some(ty) = ty {
            self.exempt.insert(ty.span().start);
        }
    }

    fn with_enclosing(&mut self, enclosing: Enclosing, decl: &TopLevelDecl) {
        let saved = self.enclosing.replace(enclosing);
        walk_top_level_decl(self, decl);
        self.enclosing = saved;
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
                .any(|ty| matches!(ty, ResolvedType::Unknown));
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
            let returned = self
                .signatures
                .instantiated_override_return(supertype, name, getter, self.model);
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
    types: impl Iterator<Item = &'t DartType>,
) -> Vec<ResolvedType> {
    types.map(|ty| model.resolve_type(ty)).collect()
}

/// `void` or `FutureOr<void>`.
fn is_void_like(ty: &ResolvedType) -> bool {
    match ty {
        ResolvedType::Void => true,
        ResolvedType::Interface {
            identity,
            arguments,
            ..
        } => {
            identity.is_sdk("dart:async", "FutureOr")
                && matches!(arguments.as_ref(), [ResolvedType::Void])
        }
        _ => false,
    }
}

impl<'a> Visitor for NullTypes<'a> {
    fn visit_top_level_decl(&mut self, node: &TopLevelDecl) {
        let model = self.model;
        let owner = |name: &Identifier| model.resolve_name(name.name.as_str());
        match node {
            TopLevelDecl::Variable(variable) => {
                self.exempt(variable.var_type.as_ref());
                walk_top_level_decl(self, node);
            }
            TopLevelDecl::Extension(extension) => {
                self.exempt(Some(&extension.on_type));
                let saved = self.enclosing.take();
                walk_top_level_decl(self, node);
                self.enclosing = saved;
            }
            TopLevelDecl::ExtensionType(extension_type) => {
                self.exempt(
                    extension_type
                        .representation()
                        .and_then(|representation| representation.param_type.as_ref()),
                );
                let enclosing = Enclosing {
                    owner: owner(&extension_type.name),
                    supertypes: resolve_all(model, extension_type.implements.iter()),
                };
                self.with_enclosing(enclosing, node);
            }
            TopLevelDecl::Class(class) => {
                let enclosing = Enclosing {
                    owner: owner(&class.name),
                    supertypes: resolve_all(
                        model,
                        class
                            .extends
                            .iter()
                            .chain(&class.with_clause)
                            .chain(&class.implements),
                    ),
                };
                self.with_enclosing(enclosing, node);
            }
            TopLevelDecl::MixinClass(class) => {
                let enclosing = Enclosing {
                    owner: owner(&class.name),
                    supertypes: resolve_all(
                        model,
                        class
                            .extends
                            .iter()
                            .chain(&class.with_clause)
                            .chain(&class.implements),
                    ),
                };
                self.with_enclosing(enclosing, node);
            }
            TopLevelDecl::Mixin(mixin) => {
                let enclosing = Enclosing {
                    owner: owner(&mixin.name),
                    supertypes: resolve_all(model, mixin.on_clause.iter().chain(&mixin.implements)),
                };
                self.with_enclosing(enclosing, node);
            }
            TopLevelDecl::Enum(enumeration) => {
                let enclosing = Enclosing {
                    owner: owner(&enumeration.name),
                    supertypes: resolve_all(
                        model,
                        enumeration
                            .with_clause
                            .iter()
                            .chain(&enumeration.implements),
                    ),
                };
                self.with_enclosing(enclosing, node);
            }
            TopLevelDecl::ClassTypeAlias(_)
            | TopLevelDecl::Function(_)
            | TopLevelDecl::TypeAlias(_)
            | TopLevelDecl::Error(_) => walk_top_level_decl(self, node),
        }
    }

    fn visit_method_decl(&mut self, node: &MethodDecl) {
        if let Some(return_type) = &node.return_type
            && self.null_name(return_type).is_some()
            && self.incompatible_override(&node.name.name, false)
        {
            self.exempt(Some(return_type));
        }
        walk_method_decl(self, node);
    }

    fn visit_getter_decl(&mut self, node: &GetterDecl) {
        if let Some(return_type) = &node.return_type
            && self.null_name(return_type).is_some()
            && self.incompatible_override(&node.name.name, true)
        {
            self.exempt(Some(return_type));
        }
        walk_getter_decl(self, node);
    }

    fn visit_local_var(&mut self, node: &LocalVarDecl) {
        self.exempt(node.var_type.as_ref());
        walk_local_var(self, node);
    }

    fn visit_expr(&mut self, node: &Expr) {
        match node {
            Expr::As { dart_type, .. } => self.exempt(Some(dart_type)),
            Expr::List {
                type_arg, elements, ..
            }
            | Expr::Set {
                type_arg, elements, ..
            } if elements.is_empty() => self.exempt(type_arg.as_ref()),
            Expr::Map {
                type_args,
                elements,
                ..
            } if elements.is_empty() => {
                for type_arg in type_args {
                    self.exempt(Some(type_arg));
                }
            }
            _ => {}
        }
        walk_expr(self, node);
    }

    fn visit_pattern(&mut self, node: &Pattern) {
        if let Pattern::Cast { cast_type, .. } = node {
            self.exempt(Some(cast_type));
        }
        walk_pattern(self, node);
    }

    fn visit_dart_type(&mut self, node: &DartType) {
        if let Some(span) = self.null_name(node)
            && !self.exempt.contains(&node.span().start)
        {
            self.spans.push(span);
        }
        if let DartType::Function(function) = node {
            if let Some(return_type) = &function.return_type {
                self.exempt(Some(return_type));
            }
            for param in &function.params {
                self.exempt(Some(&param.param_type));
            }
        }
        walk_dart_type(self, node);
    }
}
