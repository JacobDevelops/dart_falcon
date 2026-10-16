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
//! Only non-generic static methods and getters of non-generic classes and
//! extension types are checked. As in the analyzer, the last visited constructor
//! call determines whether the body creates the declared enclosing return type.
//!
//! Adopted from the official `prefer_constructors_over_static_methods` lint.

use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, ResolvedType, Rule, RulePreparation, SemanticModel,
};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr, walk_function_body};

#[derive(Default)]
pub struct PreferConstructorsOverStaticMethods;

impl Rule for PreferConstructorsOverStaticMethods {
    fn prepare<'a>(
        &self,
        program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let Some(identities) = ctx.identities else {
            return RulePreparation::Diagnostics(Vec::new());
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
                        node.primary_constructor
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
                let (name, return_type, body, is_static, generic) = match member {
                    ClassMember::Method(method) => (
                        &method.name,
                        &method.return_type,
                        &method.body,
                        method.is_static,
                        !method.type_params.is_empty(),
                    ),
                    ClassMember::Getter(getter) => (
                        &getter.name,
                        &getter.return_type,
                        &getter.body,
                        getter.is_static,
                        false,
                    ),
                    _ => continue,
                };
                let (Some(return_type), Some(body)) = (return_type, body) else {
                    continue;
                };
                if !is_static || generic {
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
                        name.span,
                    ));
                }
            }
        }
        RulePreparation::Diagnostics(diags)
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
        if let Some((identity, constructor, _)) =
            crate::lint::flutter::constructor_invocation(node, self.model)
            && (matches!(node, Expr::New { .. })
                || identity != *self.owner
                || self.constructors.contains(&constructor))
        {
            self.found = identity == *self.owner;
            if self.found {
                return;
            }
        }
        walk_expr(self, node);
    }
}
