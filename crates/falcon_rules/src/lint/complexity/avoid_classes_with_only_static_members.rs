//! Flags a class that declares only static members.
//!
//! Dart is not Java: free-standing functions and top-level variables do not
//! need a class around them. A class whose every member is static — and which
//! therefore is never instantiated for behavior of its own — only adds a
//! namespace that a library (optionally imported with a prefix) already
//! provides. Move the members to the top level instead.
//!
//! ```dart
//! class DateUtils {           // flagged
//!   static DateTime mostRecent(List<DateTime> dates) => dates.last;
//! }
//!
//! DateTime mostRecent(List<DateTime> dates) => dates.last; // preferred
//! ```
//!
//! A class holding only `static const` fields (a group of constants) is
//! allowed, as are sealed classes and classes with a non-default constructor.
//! Like the analyzer, a class is skipped when its merged interface contains an
//! instance member owned by a class other than `Object`; members replaced by
//! mixin declarations do not count. Without resolution, only local mixins are
//! considered member-free supertypes.
//!
//! Adopted from the official `avoid_classes_with_only_static_members` lint.

use falcon_analyze::{AnalyzeContext, Rule, SemanticModel, Truth};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct AvoidClassesWithOnlyStaticMembers;

impl Rule for AvoidClassesWithOnlyStaticMembers {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let model = ctx
            .identities
            .map(|identities| SemanticModel::new(ctx.file_path, identities, ctx.types));
        program
            .declarations
            .iter()
            .filter_map(|decl| {
                let (name, sealed, extends, with_clause, implements, members) = match decl {
                    TopLevelDecl::Class(class) => (
                        &class.name,
                        class.modifiers.restriction == Some(ClassRestriction::Sealed),
                        class.extends.as_ref(),
                        &class.with_clause[..],
                        &class.implements[..],
                        &class.members[..],
                    ),
                    TopLevelDecl::MixinClass(class) => (
                        &class.name,
                        false,
                        class.extends.as_ref(),
                        &class.with_clause[..],
                        &class.implements[..],
                        &class.members[..],
                    ),
                    _ => return None,
                };
                let elements = match decl {
                    TopLevelDecl::Class(class) => Some(class.element_members()),
                    TopLevelDecl::MixinClass(class) => Some(class.element_members()),
                    _ => None,
                };
                let primary = match decl {
                    TopLevelDecl::Class(class) => class.primary_constructor.as_ref(),
                    TopLevelDecl::MixinClass(class) => class.primary_constructor.as_ref(),
                    _ => None,
                };
                if elements.is_some_and(|elements| elements.declaring_params().next().is_some())
                    || primary.is_some_and(|primary| {
                        primary.constructor_name.is_some()
                            || primary.params.params.iter().any(|param| {
                                !matches!(
                                    param.kind,
                                    ParamKind::OptionalPositional
                                        | ParamKind::Named { required: false }
                                )
                            })
                    })
                {
                    return None;
                }
                if sealed || !only_static_members(members) {
                    return None;
                }
                let has_supertypes = extends.is_some_and(|ty| !is_object(ty))
                    || !with_clause.is_empty()
                    || !implements.is_empty();
                if has_supertypes {
                    let inherits = match (&model, ctx.signatures) {
                        (Some(model), Some(signatures)) => model
                            .resolve_name(&*name.name)
                            .map_or(Truth::Unknown, |identity| {
                                signatures.inherits_class_instance_member(&identity, model)
                            }),
                        _ if extends.is_none_or(is_object)
                            && implements.is_empty()
                            && with_clause.iter().all(|ty| is_local_mixin(ty, program)) =>
                        {
                            Truth::No
                        }
                        _ => Truth::Unknown,
                    };
                    if inherits != Truth::No {
                        return None;
                    }
                }
                Some(ctx.diagnostic(
                    "avoid-classes-with-only-static-members",
                    "Classes should define instance members.",
                    name.span,
                ))
            })
            .collect()
    }
}

/// Whether the declared members are all static, constructors are default
/// ones, and there is a static method or a non-const (possibly synthetic)
/// static field.
fn only_static_members(members: &[ClassMember]) -> bool {
    let mut has_method = false;
    let mut has_non_const_field = false;
    for member in members {
        match member {
            ClassMember::Constructor(constructor) => {
                if !is_default_constructor(constructor) {
                    return false;
                }
            }
            ClassMember::Method(method) => {
                if !method.is_static {
                    return false;
                }
                has_method = true;
            }
            // Static accessors induce synthetic, non-const fields.
            ClassMember::Getter(GetterDecl { is_static, .. })
            | ClassMember::Setter(SetterDecl { is_static, .. }) => {
                if !is_static {
                    return false;
                }
                has_non_const_field = true;
            }
            ClassMember::Field(field) => {
                if !field.is_static {
                    return false;
                }
                has_non_const_field |= field.keyword != VarKeyword::Const;
            }
            // A primary constructor body completes a declared constructor.
            ClassMember::Operator(_)
            | ClassMember::PrimaryConstructorBody(_)
            | ClassMember::Error(_) => return false,
        }
    }
    has_method || has_non_const_field
}

fn is_local_mixin(ty: &DartType, program: &Program) -> bool {
    matches!(ty, DartType::Named(named) if matches!(named.segments.as_slice(), [name]
        if program.declarations.iter().any(|decl| matches!(decl,
            TopLevelDecl::Mixin(mixin) if mixin.name.name == name.name))))
}

/// `extends Object` adds no supertype members.
fn is_object(ty: &DartType) -> bool {
    matches!(ty, DartType::Named(named)
        if named.type_args.is_empty()
            && matches!(named.segments.as_slice(), [segment] if segment.name == "Object"))
}

/// An unnamed constructor whose parameters are all optional.
fn is_default_constructor(constructor: &ConstructorDecl) -> bool {
    constructor.constructor_name.is_none()
        && constructor.params.params.iter().all(|param| {
            matches!(
                param.kind,
                ParamKind::OptionalPositional | ParamKind::Named { required: false }
            )
        })
}
