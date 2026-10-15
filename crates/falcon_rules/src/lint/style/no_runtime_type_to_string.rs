//! Avoid calling `toString()` on `runtimeType`.
//!
//! `runtimeType.toString()` forces the runtime to keep type names around and is
//! slow; in production builds the names may also be minified:
//!
//! ```dart
//! class Point {
//!   @override
//!   String toString() => '$runtimeType()'; // BAD
//! }
//! ```
//!
//! Reported for `this.runtimeType`, `super.runtimeType`, or an implicit-`this`
//! `runtimeType` getter that is either interpolated or has `toString()` called
//! on it. Uses inside asserts, `throw` expressions, catch clauses, mixins, and
//! abstract classes are exempt. Extension-target checks are syntactic: an
//! unqualified named target is checked only if it names a non-abstract class
//! declared in this file, `Object`, or one of the extension's type parameters.
//! Non-named targets, including `dynamic`, `void`, function and record types,
//! are checked. Falcon limitation: other named targets are exempt, including
//! prefixed targets, external classes and `Never`; this is not a semantic test
//! of whether the target is concrete.
//!
//! Adopted from the official `no_runtimeType_toString` lint.

use std::collections::HashSet;

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{
    Visitor, walk_expr, walk_formal_param, walk_function_body, walk_stmt, walk_top_level_decl,
};

#[derive(Default)]
pub struct NoRuntimeTypeToString;

impl Rule for NoRuntimeTypeToString {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let concrete_classes = program
            .declarations
            .iter()
            .filter_map(|declaration| match declaration {
                TopLevelDecl::Class(class) if !class.modifiers.is_abstract => {
                    Some(class.name.name.as_str())
                }
                TopLevelDecl::MixinClass(class) if !class.is_abstract => {
                    Some(class.name.name.as_str())
                }
                _ => None,
            })
            .collect();
        let mut collector = Collector {
            diagnostics: Vec::new(),
            ctx,
            concrete_classes,
            exempt_depth: 0,
            in_type: false,
            shadowed: false,
        };
        for declaration in &program.declarations {
            collector.visit_top_level_decl(declaration);
        }
        collector.diagnostics
    }
}

struct Collector<'a> {
    diagnostics: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
    concrete_classes: HashSet<&'a str>,
    /// Nesting depth of exempt contexts (asserts, throws, catch clauses).
    exempt_depth: usize,
    /// Inside a declaration whose instance members see a `runtimeType` getter.
    in_type: bool,
    /// The current type declares a local or parameter named `runtimeType`.
    shadowed: bool,
}

impl Collector<'_> {
    fn is_runtime_type_access(&self, expr: &Expr) -> bool {
        match expr.unparenthesized() {
            Expr::Field { object, field, .. } => {
                field.name == "runtimeType"
                    && matches!(
                        object.unparenthesized(),
                        Expr::This { .. } | Expr::Super { .. }
                    )
            }
            Expr::Ident(identifier) => {
                identifier.name == "runtimeType" && self.in_type && !self.shadowed
            }
            _ => false,
        }
    }

    fn report(&mut self, span: Span) {
        if self.exempt_depth == 0 {
            self.diagnostics.push(self.ctx.diagnostic(
                "no-runtime-type-to-string",
                "Using 'toString' on a 'Type' is not safe in production code.",
                span,
            ));
        }
    }

    fn exempt(&mut self, visit: impl FnOnce(&mut Self)) {
        self.exempt_depth += 1;
        visit(self);
        self.exempt_depth -= 1;
    }

    /// Whether an extension's `on` type is a non-abstract class, or not an
    /// interface type at all (type parameters, functions, records), in which
    /// case the official lint does not exempt it.
    fn extension_checked(&self, extension: &ExtensionDecl) -> bool {
        let DartType::Named(named) = &extension.on_type else {
            return true;
        };
        let [segment] = named.segments.as_slice() else {
            return false;
        };
        extension
            .type_params
            .iter()
            .any(|param| param.name.name == segment.name)
            || segment.name == "Object"
            || self.concrete_classes.contains(segment.name.as_str())
    }
}

impl Visitor for Collector<'_> {
    fn visit_top_level_decl(&mut self, node: &TopLevelDecl) {
        let (checked, members) = match node {
            TopLevelDecl::Class(class) => (!class.modifiers.is_abstract, class.members.as_slice()),
            TopLevelDecl::MixinClass(class) => (!class.is_abstract, class.members.as_slice()),
            TopLevelDecl::Enum(declaration) => (true, declaration.members.as_slice()),
            TopLevelDecl::ExtensionType(declaration) => (true, declaration.members.as_slice()),
            TopLevelDecl::Extension(extension) => (
                self.extension_checked(extension),
                extension.members.as_slice(),
            ),
            TopLevelDecl::Mixin(mixin) => (false, mixin.members.as_slice()),
            TopLevelDecl::ClassTypeAlias(_)
            | TopLevelDecl::Function(_)
            | TopLevelDecl::Variable(_)
            | TopLevelDecl::TypeAlias(_)
            | TopLevelDecl::Error(_) => {
                walk_top_level_decl(self, node);
                return;
            }
        };
        let mut declared = DeclaresRuntimeType(false);
        for member in members {
            declared.visit_class_member(member);
        }
        self.in_type = true;
        self.shadowed = declared.0;
        if checked {
            walk_top_level_decl(self, node);
        } else {
            self.exempt(|collector| walk_top_level_decl(collector, node));
        }
        self.in_type = false;
        self.shadowed = false;
    }

    fn visit_constructor_decl(&mut self, node: &ConstructorDecl) {
        for param in &node.params.params {
            self.visit_formal_param(param);
        }
        for initializer in &node.initializers {
            match initializer {
                ConstructorInitializer::SuperCall { args, .. }
                | ConstructorInitializer::ThisCall { args, .. } => {
                    for argument in &args.positional {
                        self.visit_expr(argument);
                    }
                    for argument in &args.named {
                        self.visit_expr(&argument.value);
                    }
                }
                ConstructorInitializer::FieldInit { value, .. } => self.visit_expr(value),
                ConstructorInitializer::Assert {
                    condition, message, ..
                } => self.exempt(|collector| {
                    collector.visit_expr(condition);
                    if let Some(message) = message {
                        collector.visit_expr(message);
                    }
                }),
            }
        }
        if let Some(body) = &node.body {
            walk_function_body(self, body);
        }
    }

    fn visit_stmt(&mut self, node: &Stmt) {
        if let Stmt::TryCatch(statement) = node {
            for nested in &statement.body.stmts {
                self.visit_stmt(nested);
            }
            for catch in &statement.catches {
                self.exempt(|collector| {
                    for nested in &catch.body.stmts {
                        collector.visit_stmt(nested);
                    }
                });
            }
            if let Some(finally) = &statement.finally {
                for nested in &finally.stmts {
                    self.visit_stmt(nested);
                }
            }
        } else if matches!(node, Stmt::Assert(_) | Stmt::Throw(_)) {
            self.exempt(|collector| walk_stmt(collector, node));
        } else {
            walk_stmt(self, node);
        }
    }

    fn visit_expr(&mut self, node: &Expr) {
        if matches!(node, Expr::Throw { .. }) {
            self.exempt(|collector| walk_expr(collector, node));
            return;
        }
        if let Expr::Call { callee, .. } = node
            && let Expr::Field { object, field, .. } = callee.unparenthesized()
            && field.name == "toString"
            && self.is_runtime_type_access(object)
        {
            self.report(field.span);
        }
        if let Expr::StringLit(literal) = node {
            for interpolation in &literal.interpolations {
                if self.is_runtime_type_access(&interpolation.expr) {
                    self.report(*interpolation.expr.span());
                }
            }
        }
        walk_expr(self, node);
    }
}

/// Finds a parameter or local variable named `runtimeType` in a declaration.
struct DeclaresRuntimeType(bool);

impl Visitor for DeclaresRuntimeType {
    fn visit_formal_param(&mut self, node: &FormalParam) {
        self.0 |= node.name.name == "runtimeType";
        walk_formal_param(self, node);
    }

    fn visit_local_var(&mut self, node: &LocalVarDecl) {
        self.0 |= node
            .declarators
            .iter()
            .any(|declarator| declarator.name.name == "runtimeType");
        falcon_syntax::visitor::walk_local_var(self, node);
    }
}
