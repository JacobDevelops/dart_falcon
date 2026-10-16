//! Flags explicit `dynamic` type annotations.
//!
//! `dynamic` opts an expression out of static type checking, so member accesses
//! and calls on it are resolved at runtime and any mistake surfaces as a crash
//! rather than a compile error. Reaching for it is usually a shortcut where a
//! concrete type, a generic, or `Object?` plus explicit checks would be safer.
//! Prefer a precise type. `dynamic` used directly as a `Map` type argument — the
//! documented JSON escape hatch, `Map<String, dynamic>` — is exempt, but a
//! `dynamic` nested more deeply (such as `Map<String, List<dynamic>>`) is still
//! flagged.
//! All explicit type positions are checked, including record fields, type
//! aliases, generic bounds, patterns, and types inside initializers.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{
    Visitor, walk_arg_list, walk_dart_type, walk_expr, walk_map_element, walk_pattern,
};

#[derive(Default)]
pub struct AvoidDynamic;

impl Rule for AvoidDynamic {
    fn prepare<'a>(
        &self,
        program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let mut visitor = DynamicVisitor {
            diagnostics: Vec::new(),
            ctx,
        };
        visitor.visit_program(program);
        RulePreparation::Diagnostics(visitor.diagnostics)
    }
}

struct DynamicVisitor<'a, 'ctx> {
    diagnostics: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'ctx>,
}

impl DynamicVisitor<'_, '_> {
    fn visit_map_arguments(&mut self, arguments: &[DartType]) {
        for argument in arguments {
            if !matches!(argument, DartType::Dynamic { .. }) {
                self.visit_dart_type(argument);
            }
        }
    }
}

impl Visitor for DynamicVisitor<'_, '_> {
    fn visit_dart_type(&mut self, node: &DartType) {
        match node {
            DartType::Dynamic { span } => self.diagnostics.push(self.ctx.diagnostic(
                "avoid-dynamic",
                "Avoid using the 'dynamic' type",
                *span,
            )),
            DartType::Named(named)
                if named
                    .segments
                    .last()
                    .is_some_and(|segment| segment.name == "Map") =>
            {
                self.visit_map_arguments(&named.type_args);
            }
            _ => walk_dart_type(self, node),
        }
    }

    fn visit_expr(&mut self, node: &Expr) {
        if let Expr::Map {
            type_args,
            elements,
            ..
        } = node
        {
            self.visit_map_arguments(type_args);
            for element in elements {
                walk_map_element(self, element);
            }
        } else if let Expr::Call {
            callee,
            type_args,
            args,
            ..
        } = node
            && is_map_name(callee)
        {
            self.visit_map_arguments(type_args);
            walk_arg_list(self, args);
        } else if let Expr::GenericInstantiation {
            target, type_args, ..
        } = node
            && is_map_name(target)
        {
            self.visit_map_arguments(type_args);
        } else {
            walk_expr(self, node);
        }
    }

    fn visit_pattern(&mut self, node: &Pattern) {
        if let Pattern::Map(map) = node {
            self.visit_map_arguments(&map.type_args);
            for entry in &map.entries {
                self.visit_expr(&entry.key);
                self.visit_pattern(&entry.pattern);
            }
        } else {
            walk_pattern(self, node);
        }
    }
}

fn is_map_name(expr: &Expr) -> bool {
    match expr.unparenthesized() {
        Expr::Ident(identifier) => identifier.name == "Map",
        Expr::Field { object, field, .. } => {
            matches!(object.unparenthesized(), Expr::Ident(_)) && field.name == "Map"
        }
        _ => false,
    }
}
