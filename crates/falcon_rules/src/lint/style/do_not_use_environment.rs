//! Disallow reading compilation environment declarations.
//!
//! Adopted from the official `do_not_use_environment` lint. Reports invocations
//! of the `dart:core` factory constructors `bool.fromEnvironment`,
//! `int.fromEnvironment`, `String.fromEnvironment`, and `bool.hasEnvironment`
//! — with or without `new`/`const`, through a `dart:core` import prefix, or as a
//! dot shorthand (`.fromEnvironment(...)`) whose context type is a declared
//! `bool`/`int`/`String` variable type. Environment-declared values make code
//! hard to understand and test. A type name the file itself declares shadows
//! the `dart:core` one and is never reported; constructor tear-offs are not
//! invocations and are not reported either. Dot shorthands in other contexts
//! (arguments, returns, default values) need a static context type falcon does
//! not compute and are skipped.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{
    Visitor, walk_expr, walk_field_decl, walk_local_var, walk_top_level_decl,
};

#[derive(Default)]
pub struct DoNotUseEnvironment;

const NAME: &str = "do-not-use-environment";
const MESSAGE: &str = "Invalid use of an environment declaration.";

impl Rule for DoNotUseEnvironment {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        if !ctx.source.contains("Environment") {
            return Vec::new();
        }
        let mut collector = Collector {
            ctx,
            core: CoreNames::new(program),
            diags: Vec::new(),
        };
        collector.visit_program(program);
        collector.diags
    }
}

/// How `dart:core` type names are reachable in this file.
struct CoreNames {
    /// Unprefixed core names are in scope (implicit `dart:core` import, or an
    /// explicit unprefixed one).
    unprefixed: bool,
    /// Prefixes of explicit `import 'dart:core' as p;` directives.
    prefixes: Vec<String>,
    /// Top-level type names declared in this file, which shadow core names.
    shadowed: Vec<String>,
}

impl CoreNames {
    fn new(program: &Program) -> Self {
        let core_imports: Vec<_> = program
            .imports
            .iter()
            .filter(|import| import.uri.value == "dart:core")
            .collect();
        let unprefixed =
            core_imports.is_empty() || core_imports.iter().any(|import| import.as_name.is_none());
        let prefixes = core_imports
            .iter()
            .filter_map(|import| import.as_name.as_ref().map(|name| name.name.clone()))
            .collect();
        let shadowed = program
            .declarations
            .iter()
            .filter_map(type_declaration_name)
            .filter(|name| is_environment_type(name))
            .map(str::to_string)
            .collect();
        Self {
            unprefixed,
            prefixes,
            shadowed,
        }
    }

    /// Whether `segments` (`[type]` or `[prefix, type]`) denote a core
    /// `bool`/`int`/`String`; returns the type name.
    fn core_type<'n>(&self, segments: &[&'n Identifier]) -> Option<&'n str> {
        match segments {
            [ty] if self.unprefixed && !self.shadowed.contains(&ty.name) => Some(ty.name.as_str()),
            [prefix, ty] if self.prefixes.contains(&prefix.name) => Some(ty.name.as_str()),
            _ => None,
        }
        .filter(|name| is_environment_type(name))
    }

    fn named_type<'n>(&self, ty: &'n DartType) -> Option<&'n str> {
        let DartType::Named(named) = ty else {
            return None;
        };
        if !named.type_args.is_empty() {
            return None;
        }
        let segments: Vec<&Identifier> = named.segments.iter().collect();
        self.core_type(&segments)
    }
}

fn type_declaration_name(decl: &TopLevelDecl) -> Option<&str> {
    let name = match decl {
        TopLevelDecl::Class(x) => &x.name,
        TopLevelDecl::ClassTypeAlias(x) => &x.name,
        TopLevelDecl::Mixin(x) => &x.name,
        TopLevelDecl::MixinClass(x) => &x.name,
        TopLevelDecl::Enum(x) => &x.name,
        TopLevelDecl::ExtensionType(x) => &x.name,
        TopLevelDecl::TypeAlias(x) => &x.name,
        TopLevelDecl::Extension(_)
        | TopLevelDecl::Function(_)
        | TopLevelDecl::Variable(_)
        | TopLevelDecl::Error(_) => return None,
    };
    Some(&name.name)
}

fn is_environment_type(name: &str) -> bool {
    matches!(name, "bool" | "int" | "String")
}

/// Whether `constructor` on core type `ty` reads the environment.
fn reads_environment(ty: &str, constructor: &str) -> bool {
    constructor == "fromEnvironment" || (ty == "bool" && constructor == "hasEnvironment")
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    core: CoreNames,
    diags: Vec<Diagnostic>,
}

impl Collector<'_> {
    fn report(&mut self, span: Span) {
        self.diags.push(self.ctx.diagnostic(NAME, MESSAGE, span));
    }

    /// Dot shorthands initializing a variable declared as a core environment type.
    fn check_shorthands(&mut self, declared: Option<&DartType>, declarators: &[VarDeclarator]) {
        let Some(ty) = declared.and_then(|ty| self.core.named_type(ty)) else {
            return;
        };
        for declarator in declarators {
            if let Some(Expr::Call { callee, .. }) = &declarator.initializer
                && let Expr::DotShorthand { name, .. } = callee.as_ref()
                && reads_environment(ty, &name.name)
            {
                self.report(name.span);
            }
        }
    }
}

impl Visitor for Collector<'_> {
    fn visit_expr(&mut self, node: &Expr) {
        match node {
            Expr::Call { callee, .. } => {
                if let Expr::Field { object, field, .. } = callee.as_ref() {
                    let segments = match object.as_ref() {
                        Expr::Ident(ty) => Some(vec![ty]),
                        Expr::Field {
                            object: prefix,
                            field: ty,
                            is_null_safe: false,
                            ..
                        } => match prefix.as_ref() {
                            Expr::Ident(prefix) => Some(vec![prefix, ty]),
                            _ => None,
                        },
                        _ => None,
                    };
                    if let Some(segments) = segments
                        && let Some(ty) = self.core.core_type(&segments)
                        && reads_environment(ty, &field.name)
                    {
                        self.report(Span {
                            start: segments[0].span.start,
                            end: field.span.end,
                        });
                    }
                }
            }
            Expr::New {
                dart_type: DartType::Named(named),
                constructor_name,
                ..
            } if named.type_args.is_empty() => {
                // `new bool.fromEnvironment(...)` may arrive with the
                // constructor name folded into the type segments.
                let mut segments: Vec<&Identifier> = named.segments.iter().collect();
                let constructor = match constructor_name {
                    Some(constructor) => Some(constructor),
                    None => segments.pop(),
                };
                if let Some(constructor) = constructor
                    && let Some(ty) = self.core.core_type(&segments)
                    && reads_environment(ty, &constructor.name)
                {
                    self.report(Span {
                        start: named.span.start,
                        end: constructor.span.end,
                    });
                }
            }
            _ => {}
        }
        walk_expr(self, node);
    }

    fn visit_local_var(&mut self, node: &LocalVarDecl) {
        self.check_shorthands(node.var_type.as_ref(), &node.declarators);
        walk_local_var(self, node);
    }

    fn visit_field_decl(&mut self, node: &FieldDecl) {
        self.check_shorthands(node.field_type.as_ref(), &node.declarators);
        walk_field_decl(self, node);
    }

    fn visit_top_level_decl(&mut self, node: &TopLevelDecl) {
        if let TopLevelDecl::Variable(variable) = node {
            self.check_shorthands(variable.var_type.as_ref(), &variable.declarators);
        }
        walk_top_level_decl(self, node);
    }
}
