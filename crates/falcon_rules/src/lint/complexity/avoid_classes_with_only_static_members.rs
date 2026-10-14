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
//! Classes with any supertype other than `Object` are skipped, since an
//! inherited instance member would make the class a real type.
//!
//! Adopted from the official `avoid_classes_with_only_static_members` lint.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct AvoidClassesWithOnlyStaticMembers;

impl Rule for AvoidClassesWithOnlyStaticMembers {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        program
            .declarations
            .iter()
            .filter_map(|decl| match decl {
                TopLevelDecl::Class(class) if only_static_members(class) => Some(ctx.diagnostic(
                    "avoid-classes-with-only-static-members",
                    "Classes should define instance members.",
                    class.name.span,
                )),
                _ => None,
            })
            .collect()
    }
}

fn only_static_members(class: &ClassDecl) -> bool {
    if class.modifiers.restriction == Some(ClassRestriction::Sealed)
        || !class.with_clause.is_empty()
        || !class.implements.is_empty()
        || class.extends.as_ref().is_some_and(|ty| !is_object(ty))
    {
        return false;
    }
    let mut has_method = false;
    let mut has_non_const_field = false;
    for member in &class.members {
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
            ClassMember::Operator(_) | ClassMember::Error(_) => return false,
        }
    }
    has_method || has_non_const_field
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
