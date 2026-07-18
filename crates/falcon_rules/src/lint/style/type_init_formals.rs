//! Flags a *redundant* type annotation on an initializing formal or super
//! parameter (`type-init-formals`, adopted from package:lints): in `C(int this.x)`
//! the annotation is redundant only when it is the same type the field already
//! declares. Annotating a *narrower* type — `num x; C(int this.x);` — is legal
//! Dart that meaningfully restricts the constructor's parameter, and upstream
//! (`nodeType.type == field.type`) does not report it.
//!
//! Falcon has no type resolver, so the comparison is made on the written type's
//! source text after whitespace normalization: `int` matches `int`, `int` does
//! not match `num`. Any case where the target type cannot be established
//! file-locally — an inherited field, a superclass declared in another file, an
//! untyped field — is skipped rather than guessed. Missing a lint is preferable
//! to reporting valid Dart.

use std::collections::HashMap;

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::Program;
use falcon_syntax::ast::*;

/// Guards the `super.x` → super-constructor-parameter walk against a cyclic
/// `extends` chain in malformed source.
const MAX_SUPER_DEPTH: usize = 8;

pub struct TypeInitFormals;

/// The file-local view of a class needed to resolve a formal's target type.
struct ClassInfo<'a> {
    members: &'a [ClassMember],
    extends: Option<&'a str>,
}

type ClassTable<'a> = HashMap<&'a str, ClassInfo<'a>>;

impl Rule for TypeInitFormals {
    fn name(&self) -> &'static str {
        "type-init-formals"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut classes: ClassTable = HashMap::new();
        for decl in &program.declarations {
            if let Some((name, info)) = class_info(decl) {
                classes.insert(name, info);
            }
        }

        let mut diags = Vec::new();
        for info in classes.values() {
            for member in info.members {
                let ClassMember::Constructor(ctor) = member else {
                    continue;
                };
                check_ctor(&classes, info, ctor, ctx, &mut diags);
            }
        }
        diags
    }
}

fn class_info(decl: &TopLevelDecl) -> Option<(&str, ClassInfo<'_>)> {
    match decl {
        TopLevelDecl::Class(c) => Some((
            c.name.name.as_str(),
            ClassInfo {
                members: &c.members,
                extends: c.extends.as_ref().and_then(type_name),
            },
        )),
        TopLevelDecl::MixinClass(m) => Some((
            m.name.name.as_str(),
            ClassInfo {
                members: &m.members,
                extends: m.extends.as_ref().and_then(type_name),
            },
        )),
        TopLevelDecl::Enum(e) => Some((
            e.name.name.as_str(),
            ClassInfo {
                members: &e.members,
                extends: None,
            },
        )),
        TopLevelDecl::ExtensionType(e) => Some((
            e.name.name.as_str(),
            ClassInfo {
                members: &e.members,
                extends: None,
            },
        )),
        _ => None,
    }
}

/// The final segment of a named type (`foo.Bar` → `Bar`); `None` for function,
/// record, and builtin types, none of which can be a superclass we index.
fn type_name(ty: &DartType) -> Option<&str> {
    match ty {
        DartType::Named(n) => n.segments.last().map(|s| s.name.as_str()),
        _ => None,
    }
}

fn check_ctor(
    classes: &ClassTable,
    class: &ClassInfo,
    ctor: &ConstructorDecl,
    ctx: &AnalyzeContext,
    diags: &mut Vec<Diagnostic>,
) {
    let positional: Vec<&FormalParam> = ctor
        .params
        .positional
        .iter()
        .chain(&ctor.params.optional_positional)
        .collect();

    // A positional `super.x` maps to the super constructor's positional
    // parameter at the same index *among super formals only* — preceding plain
    // parameters of this constructor are not forwarded.
    let mut super_index = 0;
    for param in &positional {
        let index = super_index;
        if param.is_super {
            super_index += 1;
        }
        check_param(classes, class, ctor, param, Some(index), ctx, diags);
    }
    for param in &ctor.params.named {
        check_param(classes, class, ctor, param, None, ctx, diags);
    }
}

/// `super_index` is `Some(i)` for a positional parameter (its index among this
/// constructor's positional super formals) and `None` for a named one.
fn check_param(
    classes: &ClassTable,
    class: &ClassInfo,
    ctor: &ConstructorDecl,
    param: &FormalParam,
    super_index: Option<usize>,
    ctx: &AnalyzeContext,
    diags: &mut Vec<Diagnostic>,
) {
    let Some(annotation) = &param.param_type else {
        return;
    };
    let target = if param.is_field {
        field_type(class, &param.name.name)
    } else if param.is_super {
        super_param_type(classes, class, ctor, param, super_index, MAX_SUPER_DEPTH)
    } else {
        return;
    };
    let Some(target) = target else { return };

    if same_type(ctx.source, target, annotation) {
        diags.push(Diagnostic::new(
            "type-init-formals",
            Severity::Warning,
            "Don't type annotate initializing formals.".to_string(),
            ctx.file_path.to_string_lossy().into_owned(),
            DiagSpan {
                start: annotation.span().start,
                end: annotation.span().end,
            },
        ));
    }
}

/// The declared type of instance field `name` on `class`. Dart requires an
/// initializing formal's field to be declared by the enclosing class, so no
/// supertype walk is needed.
fn field_type<'a>(class: &ClassInfo<'a>, name: &str) -> Option<&'a DartType> {
    class.members.iter().find_map(|member| match member {
        ClassMember::Field(f) if !f.is_static => f
            .declarators
            .iter()
            .any(|d| d.name.name == name)
            .then_some(f.field_type.as_ref())
            .flatten(),
        _ => None,
    })
}

/// The type a `super.x` formal inherits: the matching super-constructor
/// parameter's written type, or — when that parameter is itself an untyped
/// `this.x` / `super.x` formal — the type *it* resolves to.
fn super_param_type<'a>(
    classes: &ClassTable<'a>,
    class: &ClassInfo<'a>,
    ctor: &ConstructorDecl,
    param: &FormalParam,
    super_index: Option<usize>,
    depth: usize,
) -> Option<&'a DartType> {
    if depth == 0 {
        return None;
    }
    let super_class = classes.get(class.extends?)?;
    let super_ctor = find_ctor(super_class, super_ctor_name(ctor))?;

    let target = match super_index {
        Some(i) => super_ctor
            .params
            .positional
            .iter()
            .chain(&super_ctor.params.optional_positional)
            .nth(i)?,
        None => super_ctor
            .params
            .named
            .iter()
            .find(|p| p.name.name == param.name.name)?,
    };

    if let Some(ty) = &target.param_type {
        return Some(ty);
    }
    if target.is_field {
        return field_type(super_class, &target.name.name);
    }
    if target.is_super {
        let index = super_index.map(|_| {
            super_ctor
                .params
                .positional
                .iter()
                .chain(&super_ctor.params.optional_positional)
                .take_while(|p| !std::ptr::eq(*p, target))
                .filter(|p| p.is_super)
                .count()
        });
        return super_param_type(classes, super_class, super_ctor, target, index, depth - 1);
    }
    None
}

/// The name of the super constructor this constructor delegates to (`None` for
/// the unnamed one), from an explicit `: super.named(...)` initializer.
fn super_ctor_name(ctor: &ConstructorDecl) -> Option<&str> {
    ctor.initializers.iter().find_map(|init| match init {
        ConstructorInitializer::SuperCall { call_name, .. } => {
            call_name.as_ref().map(|n| n.name.as_str())
        }
        _ => None,
    })
}

fn find_ctor<'a>(class: &ClassInfo<'a>, name: Option<&str>) -> Option<&'a ConstructorDecl> {
    class.members.iter().find_map(|member| match member {
        ClassMember::Constructor(c) if !c.is_factory => {
            let this_name = c.constructor_name.as_ref().map(|n| n.name.as_str());
            (this_name == name).then_some(c)
        }
        _ => None,
    })
}

/// Whether two written types are the same type, compared as source text with
/// all whitespace removed. Sound in the direction that matters: differing text
/// (`num` vs `int`) is never reported.
fn same_type(source: &str, a: &DartType, b: &DartType) -> bool {
    let text = |ty: &DartType| {
        let span = ty.span();
        source
            .get(span.start..span.end)
            .map(|s| s.split_whitespace().collect::<String>())
    };
    match (text(a), text(b)) {
        (Some(a), Some(b)) => a == b,
        _ => false,
    }
}
