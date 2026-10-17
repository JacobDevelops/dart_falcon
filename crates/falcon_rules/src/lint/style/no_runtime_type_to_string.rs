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

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralNode, StructuralRuleVisitor,
    StructuralTraversal, structural_visitor,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct NoRuntimeTypeToString;

impl Rule for NoRuntimeTypeToString {
    fn prepare<'a>(
        &self,
        program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
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
        RulePreparation::Structural(Box::new(Collector {
            diagnostics: Vec::new(),
            ctx,
            concrete_classes,
            exempt_depth: 0,
            in_type: false,
            shadowed: false,
            member_depth: 0,
            pending: Vec::new(),
            constructor_depth: 0,
        }))
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
    member_depth: usize,
    pending: Vec<(Span, bool)>,
    constructor_depth: usize,
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
            Expr::Ident(identifier) => identifier.name == "runtimeType" && self.in_type,
            _ => false,
        }
    }

    fn report(&mut self, span: Span, implicit: bool) {
        if self.exempt_depth == 0 {
            self.pending.push((span, implicit));
        }
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

structural_visitor! {
impl StructuralRuleVisitor for Collector<'_> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diagnostics }

    fn route_child(&mut self, parent: StructuralNode<'_>, child: StructuralNode<'_>) -> bool {
        match parent {
            StructuralNode::Program(_) => matches!(child, StructuralNode::TopLevelDecl(_)),
            StructuralNode::ConstructorDecl(_) => !matches!(
                child, StructuralNode::Annotation(_) | StructuralNode::RedirectedConstructor(_)
            ),
            _ => true,
        }
    }

    fn visit_top_level_decl(&mut self, node: &TopLevelDecl) -> StructuralTraversal {
        let checked = match node {
            TopLevelDecl::Class(class) => Some(!class.modifiers.is_abstract),
            TopLevelDecl::MixinClass(class) => Some(!class.is_abstract),
            TopLevelDecl::Enum(_) | TopLevelDecl::ExtensionType(_) => Some(true),
            TopLevelDecl::Extension(extension) => Some(self.extension_checked(extension)),
            TopLevelDecl::Mixin(_) => Some(false),
            _ => None,
        };
        self.in_type = checked.is_some();
        self.exempt_depth += usize::from(checked == Some(false));
        StructuralTraversal::Continue
    }

    fn exit_top_level_decl(&mut self, _node: &TopLevelDecl) {
        for (span, implicit) in self.pending.drain(..) {
            if !implicit || !self.shadowed {
                self.diagnostics.push(self.ctx.diagnostic(
                    "no-runtime-type-to-string",
                    "Using 'toString' on a 'Type' is not safe in production code.",
                    span,
                ));
            }
        }
        self.in_type = false;
        self.shadowed = false;
        self.exempt_depth = 0;
    }

    fn visit_constructor_decl(&mut self, _node: &ConstructorDecl) -> StructuralTraversal {
        self.constructor_depth += 1;
        StructuralTraversal::Continue
    }
    fn exit_constructor_decl(&mut self, _node: &ConstructorDecl) { self.constructor_depth -= 1; }

    fn visit_constructor_initializer(&mut self, node: &ConstructorInitializer) -> StructuralTraversal {
        self.exempt_depth += usize::from(self.constructor_depth > 0 && matches!(node, ConstructorInitializer::Assert { .. }));
        StructuralTraversal::Continue
    }
    fn exit_constructor_initializer(&mut self, node: &ConstructorInitializer) {
        self.exempt_depth -= usize::from(self.constructor_depth > 0 && matches!(node, ConstructorInitializer::Assert { .. }));
    }

    fn visit_catch_clause(&mut self, _node: &CatchClause) -> StructuralTraversal {
        self.exempt_depth += 1;
        StructuralTraversal::Continue
    }
    fn exit_catch_clause(&mut self, _node: &CatchClause) { self.exempt_depth -= 1; }

    fn visit_class_member(&mut self, _node: &ClassMember) -> StructuralTraversal {
        self.member_depth += 1;
        StructuralTraversal::Continue
    }
    fn exit_class_member(&mut self, _node: &ClassMember) { self.member_depth -= 1; }

    fn visit_formal_param(&mut self, node: &FormalParam) -> StructuralTraversal {
        self.shadowed |= self.member_depth > 0 && node.name.name == "runtimeType";
        StructuralTraversal::Continue
    }
    fn visit_local_var(&mut self, node: &LocalVarDecl) -> StructuralTraversal {
        self.shadowed |= self.member_depth > 0 && node.declarators.iter().any(|declarator| declarator.name.name == "runtimeType");
        StructuralTraversal::Continue
    }

    fn visit_stmt(&mut self, node: &Stmt) -> StructuralTraversal {
        self.exempt_depth += usize::from(matches!(node, Stmt::Assert(_) | Stmt::Throw(_)));
        StructuralTraversal::Continue
    }
    fn exit_stmt(&mut self, node: &Stmt) {
        self.exempt_depth -= usize::from(matches!(node, Stmt::Assert(_) | Stmt::Throw(_)));
    }

    fn visit_expr(&mut self, node: &Expr) -> StructuralTraversal {
        self.exempt_depth += usize::from(matches!(node, Expr::Throw { .. }));
        if let Expr::Call { callee, .. } = node
            && let Expr::Field { object, field, .. } = callee.unparenthesized()
            && field.name == "toString"
            && self.is_runtime_type_access(object)
        {
            self.report(field.span, matches!(object.unparenthesized(), Expr::Ident(_)));
        }
        if let Expr::StringLit(literal) = node {
            for interpolation in &literal.interpolations {
                if self.is_runtime_type_access(&interpolation.expr) {
                    self.report(*interpolation.expr.span(), matches!(interpolation.expr.unparenthesized(), Expr::Ident(_)));
                }
            }
        }
        StructuralTraversal::Continue
    }
    fn exit_expr(&mut self, node: &Expr) {
        self.exempt_depth -= usize::from(matches!(node, Expr::Throw { .. }));
    }
}
}

#[cfg(test)]
mod tests {
    use super::*;
    use falcon_analyze::RuleAnalysis;
    use falcon_config::FalconConfig;
    use std::path::Path;

    #[test]
    fn only_ordinary_constructor_assert_initializers_are_exempt() {
        let source = "class Ordinary { Ordinary() : assert(runtimeType.toString().isNotEmpty); } class Primary(int value) { this : assert(runtimeType.toString().isNotEmpty); }";
        let parsed = falcon_dart_parser::parse(source);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let config = FalconConfig::default();
        let ctx = AnalyzeContext::new(Path::new("test.dart"), source, &config, &parsed.syntax);
        let diagnostics = NoRuntimeTypeToString.analyze(&parsed.program, &ctx);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].span.start, source.rfind("toString").unwrap());
    }
}
