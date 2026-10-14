//! Requires explicit type annotations everywhere a type could be inferred.
//!
//! This is the opposite of `omit-local-variable-types`: every variable,
//! loop variable, pattern variable, and parameter needs a declared type;
//! collection literals need explicit type arguments; and generic types need
//! their type arguments spelled out instead of defaulting to `dynamic`.
//!
//! ```dart
//! var count = 0;           // flagged
//! final names = [];        // flagged (twice: declaration and literal)
//! List items = <int>[];    // flagged: raw generic type
//! int count = 0;           // ok
//! ```
//!
//! Raw generic types are only reported for types whose type parameters are
//! known: SDK collection and async types, and classes, mixins, enums, and type
//! aliases declared in the same file. `is` checks, `FutureOr`, and types
//! annotated `@optionalTypeArgs` are exempt.
//!
//! Adopted from the official `always_specify_types` lint.

use std::collections::HashMap;

use falcon_analyze::{AnalyzeContext, DeclarationIdentity, Rule, SemanticModel};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{
    Visitor, walk_dart_type, walk_expr, walk_field_decl, walk_formal_param, walk_local_var,
    walk_pattern, walk_setter_decl, walk_stmt, walk_top_level_decl,
};

const RULE: &str = "always-specify-types";
const MESSAGE: &str = "Missing type annotation.";

#[derive(Default)]
pub struct AlwaysSpecifyTypes;

impl Rule for AlwaysSpecifyTypes {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut collector = Collector {
            ctx,
            model: ctx
                .identities
                .map(|identities| SemanticModel::new(ctx.file_path, identities, ctx.types)),
            local_generics: local_generic_types(program),
            is_type_test: None,
            diags: Vec::new(),
        };
        collector.visit_program(program);
        collector.diags
    }
}

/// The number of type parameters of an SDK or allowlisted package type, which
/// the resolver models by identity only.
pub(crate) fn known_type_arity(identity: &DeclarationIdentity) -> Option<usize> {
    match identity {
        DeclarationIdentity::Sdk { library, name } => Some(match (*library, *name) {
            (
                "dart:core",
                "Comparable" | "Iterable" | "Iterator" | "List" | "Set" | "Expando"
                | "WeakReference" | "Finalizer",
            )
            | ("dart:async", "Future" | "FutureOr" | "Completer" | "StreamController")
            | (
                "dart:collection",
                "Queue" | "ListQueue" | "DoubleLinkedQueue" | "HashSet" | "LinkedHashSet"
                | "SplayTreeSet",
            )
            | ("dart:js_interop", "JSArray" | "JSPromise") => 1,
            ("dart:core", "Map" | "MapEntry")
            | ("dart:collection", "HashMap" | "LinkedHashMap" | "SplayTreeMap") => 2,
            _ => 0,
        }),
        DeclarationIdentity::Package { package, name } => {
            Some(usize::from(*package == "flutter" && *name == "State"))
        }
        DeclarationIdentity::Project { .. } => None,
    }
}

/// SDK and package generics the lint exempts: `FutureOr` is not an interface
/// type, and Flutter's `State` and the `dart:js_interop` generics are
/// `@optionalTypeArgs`-style types whose arguments are commonly omitted.
fn exempt_known_generic(identity: &DeclarationIdentity) -> bool {
    matches!(
        identity,
        DeclarationIdentity::Sdk {
            library: "dart:async",
            name: "FutureOr"
        } | DeclarationIdentity::Sdk {
            library: "dart:js_interop",
            ..
        } | DeclarationIdentity::Package { .. }
    )
}

#[derive(Default)]
struct LocalGeneric<'p> {
    constructors: Vec<&'p str>,
    has_unnamed_constructor: bool,
}

/// Generic types declared in this file without `@optionalTypeArgs`.
fn local_generic_types(program: &Program) -> HashMap<&str, LocalGeneric<'_>> {
    let optional = |annotations: &[Annotation]| {
        annotations.iter().any(|annotation| {
            annotation
                .name
                .last()
                .is_some_and(|name| name.name == "optionalTypeArgs")
        })
    };
    let mut generics = HashMap::new();
    for declaration in &program.declarations {
        let (name, annotations, type_params, members): (_, _, _, &[ClassMember]) = match declaration
        {
            TopLevelDecl::Class(c) => (&c.name, &c.annotations, &c.type_params, &c.members),
            TopLevelDecl::MixinClass(c) => (&c.name, &c.annotations, &c.type_params, &c.members),
            TopLevelDecl::Mixin(m) => (&m.name, &m.annotations, &m.type_params, &m.members),
            TopLevelDecl::Enum(e) => (&e.name, &e.annotations, &e.type_params, &[]),
            TopLevelDecl::TypeAlias(alias) if matches!(alias.aliased, DartType::Named(_)) => {
                (&alias.name, &alias.annotations, &alias.type_params, &[])
            }
            _ => continue,
        };
        if type_params.is_empty() || optional(annotations) {
            continue;
        }
        let mut generic = LocalGeneric::default();
        let mut declares_constructor = false;
        for member in members {
            if let ClassMember::Constructor(constructor) = member {
                declares_constructor = true;
                match &constructor.constructor_name {
                    Some(named) => generic.constructors.push(named.name.as_str()),
                    None => generic.has_unnamed_constructor = true,
                }
            }
        }
        generic.has_unnamed_constructor |=
            !declares_constructor && matches!(declaration, TopLevelDecl::Class(_));
        generics.insert(name.name.as_str(), generic);
    }
    generics
}

struct Collector<'a, 'ctx, 'p> {
    ctx: &'a AnalyzeContext<'ctx>,
    model: Option<SemanticModel<'ctx>>,
    local_generics: HashMap<&'p str, LocalGeneric<'p>>,
    /// The type tested by the innermost `is` expression, which may stay raw.
    is_type_test: Option<Span>,
    diags: Vec<Diagnostic>,
}

impl Collector<'_, '_, '_> {
    fn report(&mut self, span: Span) {
        self.diags.push(self.ctx.diagnostic(RULE, MESSAGE, span));
    }

    /// Whether a type name without type arguments leaves generic parameters
    /// to default.
    fn is_raw_generic(&self, segments: &[Identifier]) -> bool {
        if let [segment] = segments
            && self.local_generics.contains_key(segment.name.as_str())
        {
            return true;
        }
        let Some(model) = &self.model else {
            return false;
        };
        let Some(name) = falcon_analyze::QualifiedName::from_identifiers(segments) else {
            return false;
        };
        model.resolve_name(name).is_some_and(|identity| {
            !exempt_known_generic(&identity) && known_type_arity(&identity).is_some_and(|n| n > 0)
        })
    }

    /// The `var`/`final`/`const` keyword within `start..end`.
    fn keyword(&self, start: usize, end: usize) -> Span {
        let text = self.ctx.source.get(start..end).unwrap_or_default();
        let bytes = text.as_bytes();
        let boundary = |at: usize| {
            bytes
                .get(at)
                .is_none_or(|b| !(b.is_ascii_alphanumeric() || *b == b'_' || *b == b'$'))
        };
        ["var", "final", "const"]
            .iter()
            .filter_map(|word| {
                text.match_indices(word)
                    .find(|(at, _)| (*at == 0 || boundary(at - 1)) && boundary(at + word.len()))
                    .map(|(at, _)| Span::new(start + at, start + at + word.len()))
            })
            .min_by_key(|span| span.start)
            .unwrap_or(Span::new(start, end))
    }

    fn check_declaration(
        &mut self,
        annotations: &[Annotation],
        span: Span,
        var_type: Option<&DartType>,
        declarators: &[VarDeclarator],
    ) {
        if var_type.is_some() {
            return;
        }
        let Some(first) = declarators.first() else {
            return;
        };
        let start = annotations
            .last()
            .map_or(span.start, |annotation| annotation.span.end);
        let keyword = self.keyword(start, first.name.span.start);
        self.report(keyword);
    }

    /// `for (var x in xs)` / `for (final x in xs)`: the keyword directly
    /// precedes the loop variable name.
    fn check_for_in(&mut self, var_type: Option<&DartType>, name: &Identifier) {
        if var_type.is_some() {
            return;
        }
        let before = self
            .ctx
            .source
            .get(..name.span.start)
            .unwrap_or_default()
            .trim_end();
        for word in ["var", "final"] {
            if let Some(head) = before.strip_suffix(word)
                && head
                    .chars()
                    .next_back()
                    .is_none_or(|c| !(c.is_alphanumeric() || c == '_' || c == '$'))
            {
                self.report(Span::new(head.len(), before.len()));
                return;
            }
        }
    }

    fn check_untyped_param(&mut self, node: &FormalParam) {
        if node.param_type.is_none()
            && node.initializing.is_none()
            && node.function_params.is_none()
            && !node.name.name.chars().all(|c| c == '_')
        {
            self.report(node.name.span);
        }
    }

    /// `Box(1)` / `Box.named(1)`: a constructor of a generic class declared
    /// in this file, invoked without type arguments.
    fn raw_generic_construction(&self, callee: &Expr) -> Option<Span> {
        let (class, constructor) = match callee {
            Expr::Ident(class) => (class, None),
            Expr::Field { object, field, .. } => match object.as_ref() {
                Expr::Ident(class) => (class, Some(field.name.as_str())),
                _ => return None,
            },
            _ => return None,
        };
        let generic = self.local_generics.get(class.name.as_str())?;
        let constructs = match constructor {
            None => generic.has_unnamed_constructor,
            Some(name) => generic.constructors.contains(&name),
        };
        constructs.then_some(class.span)
    }

    fn check_pattern_shorthands(&mut self, pattern: &Pattern) {
        if let Pattern::Object(object) = pattern {
            for field in &object.fields {
                if field.pattern.is_none() {
                    self.report(field.name.span);
                }
            }
        }
    }
}

impl Visitor for Collector<'_, '_, '_> {
    fn visit_top_level_decl(&mut self, node: &TopLevelDecl) {
        match node {
            TopLevelDecl::Variable(variable) => self.check_declaration(
                &variable.annotations,
                variable.span,
                variable.var_type.as_ref(),
                &variable.declarators,
            ),
            // Legacy `typedef F(x);` parameters without a type get a synthesized
            // `dynamic` spanning the parameter itself.
            TopLevelDecl::TypeAlias(alias) => {
                if let DartType::Function(function) = &alias.aliased
                    && function.span.start <= alias.name.span.start
                {
                    for param in &function.params {
                        if let (DartType::Dynamic { span }, Some(name)) =
                            (&param.param_type, &param.name)
                            && span.start == name.span.start
                        {
                            self.report(name.span);
                        }
                    }
                }
            }
            _ => {}
        }
        walk_top_level_decl(self, node);
    }

    fn visit_field_decl(&mut self, node: &FieldDecl) {
        self.check_declaration(
            &node.annotations,
            node.span,
            node.field_type.as_ref(),
            &node.declarators,
        );
        walk_field_decl(self, node);
    }

    fn visit_local_var(&mut self, node: &LocalVarDecl) {
        self.check_declaration(&[], node.span, node.var_type.as_ref(), &node.declarators);
        walk_local_var(self, node);
    }

    fn visit_formal_param(&mut self, node: &FormalParam) {
        self.check_untyped_param(node);
        walk_formal_param(self, node);
    }

    fn visit_setter_decl(&mut self, node: &SetterDecl) {
        if node.param_type.is_none() {
            self.report(node.param.span);
        }
        walk_setter_decl(self, node);
    }

    fn visit_stmt(&mut self, node: &Stmt) {
        match node {
            Stmt::For(ForStmt {
                header:
                    ForHeader {
                        init: Some(ForInit::ForIn { var_type, name, .. }),
                        ..
                    },
                ..
            }) => self.check_for_in(var_type.as_ref(), name),
            // `(a, b) = r;` assigns existing variables; nothing is declared.
            Stmt::PatternAssign(assign) => {
                self.visit_expr(&assign.value);
                return;
            }
            _ => {}
        }
        walk_stmt(self, node);
    }

    fn visit_pattern(&mut self, node: &Pattern) {
        if let Pattern::Variable {
            type_: None,
            name,
            span,
        } = node
            && name.name != "_"
        {
            self.report(*span);
        }
        self.check_pattern_shorthands(node);
        walk_pattern(self, node);
    }

    fn visit_dart_type(&mut self, node: &DartType) {
        if let DartType::Named(named) = node
            && named.type_args.is_empty()
            && self.is_type_test != Some(named.span)
            && self.is_raw_generic(&named.segments)
        {
            self.report(named.span);
        }
        walk_dart_type(self, node);
    }

    fn visit_expr(&mut self, node: &Expr) {
        match node {
            Expr::List {
                type_arg: None,
                span,
                ..
            }
            | Expr::Set {
                type_arg: None,
                span,
                ..
            } => self.report(*span),
            Expr::Map {
                type_args, span, ..
            } if type_args.is_empty() => self.report(*span),
            // The tested type of an `is` check may stay raw; its arguments may not.
            Expr::Is { dart_type, .. } => self.is_type_test = Some(*dart_type.span()),
            Expr::Call {
                callee, type_args, ..
            } if type_args.is_empty() => {
                if let Some(span) = self.raw_generic_construction(callee) {
                    self.report(span);
                }
            }
            _ => {}
        }
        walk_expr(self, node);
    }
}
