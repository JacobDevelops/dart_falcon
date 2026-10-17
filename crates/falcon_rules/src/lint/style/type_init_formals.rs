//! Flags a *redundant* type annotation on an initializing formal or super
//! parameter (`type-init-formals`, adopted from package:lints).
//!
//! In `C(int this.x)` or `C(int super.x)` the annotation restates a type the
//! field (or super-parameter) already fixes, so it adds nothing and risks
//! drifting out of sync with that declaration. It is redundant only when it
//! names the *same* type: annotating a narrower one — `num x; C(int this.x);` —
//! is legal Dart that meaningfully restricts the constructor's parameter, and
//! upstream (`nodeType.type == field.type`) does not report it.
//!
//! Falcon limitation: this rule compares written types after whitespace
//! normalization, not semantic type equality. Target types come from explicitly
//! declared file-local fields and non-primary superclass constructors, including
//! forwarded super parameters. Primary-constructor fields and extension-type
//! representation fields are not visited. Missing or untyped targets are skipped.
//! Superclass lookup uses only the final name segment, so a prefixed superclass
//! can be confused with a local declaration. Likewise, identically spelled type
//! parameters from different scopes can compare equal without representing the
//! same type. A reported annotation is therefore not always safe to remove.

use std::collections::HashMap;

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

/// Guards the `super.x` → super-constructor-parameter walk against a cyclic
/// `extends` chain in malformed source.
const MAX_SUPER_DEPTH: usize = 8;

#[derive(Default)]
pub struct TypeInitFormals;

/// The file-local view of a class needed to resolve a formal's target type.
struct ClassInfo<'a> {
    members: AstChildren<'a, ClassMember<'a>>,
    id: NodeId,
    extends: Option<&'a str>,
}

type ClassTable<'a> = HashMap<&'a str, ClassInfo<'a>>;

impl Rule for TypeInitFormals {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let mut classes: ClassTable = HashMap::new();
        for decl in program.declarations() {
            if let Some((name, info)) = class_info(decl) {
                classes.insert(name, info);
            }
        }

        RulePreparation::Structural(Box::new(Collector {
            ctx,
            classes,
            diags: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    classes: ClassTable<'a>,
    diags: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_top_level_decl(&mut self, decl: TopLevelDecl<'a>) -> StructuralTraversal {
        // The dependency table keeps only the last declaration of each name.
        // Check that declaration once, not once per overwritten declaration.
        if let Some((name, declaration)) = class_info(decl)
            && let Some(info) = self.classes.get(name)
            && info.id == declaration.id
        {
            for member in info.members.clone() {
                if let ClassMember::Constructor(ctor) = member {
                    check_ctor(&self.classes, info, ctor, self.ctx, &mut self.diags);
                }
            }
        }
        StructuralTraversal::SkipChildren
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }
}
}

fn class_info(decl: TopLevelDecl<'_>) -> Option<(&str, ClassInfo<'_>)> {
    match decl {
        TopLevelDecl::Class(c) => Some((
            c.name().name(),
            ClassInfo {
                members: c.members(),
                id: c.syntax().id(),
                extends: c.extends().and_then(type_name),
            },
        )),
        TopLevelDecl::MixinClass(m) => Some((
            m.name().name(),
            ClassInfo {
                members: m.members(),
                id: m.syntax().id(),
                extends: m.extends().and_then(type_name),
            },
        )),
        TopLevelDecl::Enum(e) => Some((
            e.name().name(),
            ClassInfo {
                members: e.members(),
                id: e.syntax().id(),
                extends: None,
            },
        )),
        TopLevelDecl::ExtensionType(e) => Some((
            e.name().name(),
            ClassInfo {
                members: e.members(),
                id: e.syntax().id(),
                extends: None,
            },
        )),
        _ => None,
    }
}

/// The final segment of a named type (`foo.Bar` → `Bar`); `None` for function,
/// record, and builtin types, none of which can be a superclass we index.
fn type_name(ty: DartType<'_>) -> Option<&str> {
    match ty {
        DartType::Named(n) => n.segments().last().map(|s| s.name()),
        _ => None,
    }
}

fn check_ctor(
    classes: &ClassTable,
    class: &ClassInfo,
    ctor: ConstructorDecl<'_>,
    ctx: &AnalyzeContext,
    diags: &mut Vec<Diagnostic>,
) {
    // A positional `super.x` maps to the super constructor's positional
    // parameter at the same index *among super formals only* — preceding plain
    // parameters of this constructor are not forwarded.
    let mut super_index = 0;
    for param in ctor.params().positional() {
        let index = super_index;
        if param.initializing() == Some(InitializingFormal::Super) {
            super_index += 1;
        }
        check_param(classes, class, ctor, param, Some(index), ctx, diags);
    }
    for param in ctor.params().named() {
        check_param(classes, class, ctor, param, None, ctx, diags);
    }
}

/// `super_index` is `Some(i)` for a positional parameter (its index among this
/// constructor's positional super formals) and `None` for a named one.
fn check_param(
    classes: &ClassTable,
    class: &ClassInfo,
    ctor: ConstructorDecl<'_>,
    param: FormalParam<'_>,
    super_index: Option<usize>,
    ctx: &AnalyzeContext,
    diags: &mut Vec<Diagnostic>,
) {
    let Some(annotation) = param.param_type() else {
        return;
    };
    let target = if param.initializing() == Some(InitializingFormal::This) {
        field_type(class, param.name().name())
    } else if param.initializing() == Some(InitializingFormal::Super) {
        super_param_type(classes, class, ctor, param, super_index, MAX_SUPER_DEPTH)
    } else {
        return;
    };
    let Some(target) = target else { return };

    if same_type(ctx, target, annotation) {
        diags.push(ctx.diagnostic(
            "type-init-formals",
            "Don't type annotate initializing formals.".to_string(),
            annotation.span(),
        ));
    }
}

/// The declared type of instance field `name` on `class`. Dart requires an
/// initializing formal's field to be declared by the enclosing class, so no
/// supertype walk is needed.
fn field_type<'a>(class: &ClassInfo<'a>, name: &str) -> Option<DartType<'a>> {
    class.members.clone().find_map(|member| match member {
        ClassMember::Field(f) if !f.is_static() => f
            .declarators()
            .any(|d| d.name().name() == name)
            .then(|| f.field_type())
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
    ctor: ConstructorDecl<'_>,
    param: FormalParam<'_>,
    super_index: Option<usize>,
    depth: usize,
) -> Option<DartType<'a>> {
    if depth == 0 {
        return None;
    }
    let super_class = classes.get(class.extends?)?;
    let super_ctor = find_ctor(super_class, super_ctor_name(ctor))?;

    let target = match super_index {
        Some(i) => super_ctor.params().positional().nth(i)?,
        None => super_ctor
            .params()
            .named()
            .find(|p| p.name().name() == param.name().name())?,
    };

    if let Some(ty) = target.param_type() {
        return Some(ty);
    }
    if target.initializing() == Some(InitializingFormal::This) {
        return field_type(super_class, target.name().name());
    }
    if target.initializing() == Some(InitializingFormal::Super) {
        let index = super_index.map(|position| {
            super_ctor
                .params()
                .positional()
                .take(position)
                .filter(|p| p.initializing() == Some(InitializingFormal::Super))
                .count()
        });
        return super_param_type(classes, super_class, super_ctor, target, index, depth - 1);
    }
    None
}

/// The name of the super constructor this constructor delegates to (`None` for
/// the unnamed one), from an explicit `: super.named(...)` initializer.
fn super_ctor_name(ctor: ConstructorDecl<'_>) -> Option<&str> {
    ctor.initializers().find_map(|init| match init {
        ConstructorInitializer::SuperCall(call) => call.call_name().map(|n| n.name()),
        _ => None,
    })
}

fn find_ctor<'a>(class: &ClassInfo<'a>, name: Option<&str>) -> Option<ConstructorDecl<'a>> {
    class.members.clone().find_map(|member| match member {
        ClassMember::Constructor(c) if !c.is_factory() => {
            let this_name = c.constructor_name().map(|n| n.name());
            (this_name == name).then_some(c)
        }
        _ => None,
    })
}

/// Compares written type text with whitespace removed, without resolving scope
/// or substituting type parameters. Equal spellings need not denote equal types.
fn same_type(ctx: &AnalyzeContext, a: DartType<'_>, b: DartType<'_>) -> bool {
    let text = |ty: DartType<'_>| {
        ctx.tree()
            .tokens_in(ctx.source, ty.span())
            .flat_map(move |token| token.text().chars())
            .filter(|character| !character.is_whitespace())
    };
    text(a).eq(text(b))
}

#[cfg(test)]
mod tests {
    use super::*;
    use falcon_analyze::RuleAnalysis;

    #[test]
    fn duplicate_class_names_check_only_the_last_declaration_once() {
        let source = "class C { int x; C(int this.x); }\nclass C { int y; C(int this.y); }";
        let falcon_dart_parser::ParseResult { tree, errors } = falcon_dart_parser::parse(source);
        assert!(errors.is_empty(), "{errors:?}");
        let parsed = std::sync::Arc::new(falcon_analyze::ParsedFile {
            source: source.into(),
            tree,
        });
        let config = falcon_config::FalconConfig::default();
        let context = AnalyzeContext::new(std::path::Path::new("test.dart"), &config, &parsed);
        let diagnostics = TypeInitFormals.analyze(parsed.program(), &context);
        assert_eq!(diagnostics.len(), 1);
        let diagnostic = &diagnostics[0];
        assert_eq!(&source[diagnostic.span.start..diagnostic.span.end], "int");
        assert!(diagnostic.span.start > source.find('\n').unwrap());
    }
}
