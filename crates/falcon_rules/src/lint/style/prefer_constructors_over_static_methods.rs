//! Flags static methods that create and return an instance of their own class.
//!
//! A static method whose job is to build an instance of the enclosing type is a
//! named constructor in disguise. Named (and factory) constructors make the
//! intent explicit, show up as constructors in tooling, and work with
//! constructor tear-offs.
//!
//! ```dart
//! class Point {
//!   final num x, y;
//!   Point(this.x, this.y);
//!   static Point polar(num theta, num radius) =>   // flagged
//!       Point(radius * cos(theta), radius * sin(theta));
//!   Point.fromPolar(num theta, num radius)         // preferred
//!       : x = radius * cos(theta), y = radius * sin(theta);
//! }
//! ```
//!
//! Only non-generic static methods of non-generic classes and extension types
//! are checked, with the enclosing type (non-nullable) as declared return type
//! and a constructor call of that type somewhere in the body.
//!
//! Adopted from the official `prefer_constructors_over_static_methods` lint.

use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, ResolvedType, Rule, SemanticModel,
    resolve_call_constructor,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr, walk_function_body};

#[derive(Default)]
pub struct PreferConstructorsOverStaticMethods;

impl Rule for PreferConstructorsOverStaticMethods {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(identities) = ctx.identities else {
            return Vec::new();
        };
        let model = SemanticModel::new(ctx.file_path, identities, ctx.types);
        let mut diags = Vec::new();
        for decl in &program.declarations {
            let (name, type_params, members, representation) = match decl {
                TopLevelDecl::Class(node) => (&node.name, &node.type_params, &node.members, None),
                TopLevelDecl::MixinClass(node) => {
                    (&node.name, &node.type_params, &node.members, None)
                }
                TopLevelDecl::ExtensionType(node) => (
                    &node.name,
                    &node.type_params,
                    &node.members,
                    Some(
                        node.representation
                            .constructor_name
                            .as_ref()
                            .map_or("new", |name| name.name.as_str()),
                    ),
                ),
                _ => continue,
            };
            if !type_params.is_empty() {
                continue;
            }
            let Some(owner) = model.resolve_name(name.name.as_str()) else {
                continue;
            };
            let mut constructors: Vec<&str> = members
                .iter()
                .filter_map(|member| match member {
                    ClassMember::Constructor(constructor) => Some(
                        constructor
                            .constructor_name
                            .as_ref()
                            .map_or("new", |name| name.name.as_str()),
                    ),
                    _ => None,
                })
                .chain(representation)
                .collect();
            if constructors.is_empty() {
                // The implicit default constructor.
                constructors.push("new");
            }
            for member in members {
                let ClassMember::Method(method) = member else {
                    continue;
                };
                let (Some(return_type), Some(body)) = (&method.return_type, &method.body) else {
                    continue;
                };
                if !method.is_static || !method.type_params.is_empty() {
                    continue;
                }
                let returns_owner = matches!(
                    model.resolve_type(return_type),
                    ResolvedType::Interface { identity, nullable: false, .. } if identity == owner
                );
                if !returns_owner {
                    continue;
                }
                let mut finder = InstanceCreation {
                    owner: &owner,
                    constructors: &constructors,
                    model: &model,
                    found: false,
                };
                walk_function_body(&mut finder, body);
                if finder.found {
                    diags.push(ctx.diagnostic(
                        "prefer-constructors-over-static-methods",
                        "Static method should be a constructor.",
                        method.name.span,
                    ));
                }
            }
        }
        diags
    }
}

/// Finds a constructor invocation of the owner type.
struct InstanceCreation<'a> {
    owner: &'a DeclarationIdentity,
    constructors: &'a [&'a str],
    model: &'a SemanticModel<'a>,
    found: bool,
}

impl Visitor for InstanceCreation<'_> {
    fn visit_expr(&mut self, node: &Expr) {
        if self.found {
            return;
        }
        self.found = match node {
            Expr::New { dart_type, .. } => matches!(
                self.model.resolve_type(dart_type),
                ResolvedType::Interface { identity, .. } if identity == *self.owner
            ),
            Expr::Call { callee, .. } => resolve_call_constructor(callee, self.model, false)
                .is_some_and(|(identity, constructor)| {
                    identity == *self.owner && self.constructors.contains(&constructor)
                }),
            _ => false,
        };
        walk_expr(self, node);
    }
}
