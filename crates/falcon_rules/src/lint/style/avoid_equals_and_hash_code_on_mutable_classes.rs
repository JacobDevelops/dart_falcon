//! Flags `==` and `hashCode` overrides in classes not marked `@immutable`.
//!
//! A value-based `==` and `hashCode` on a mutable object is a trap: once the
//! object is stored in a `Set` or used as a `Map` key, mutating it changes its
//! hash code and the collection silently loses track of it. Only classes that
//! are (transitively) annotated with package:meta's `@immutable` should
//! override them.
//!
//! ```dart
//! class Point {                 // not @immutable
//!   int x;
//!   Point(this.x);
//!   @override
//!   bool operator ==(Object other) => other is Point && other.x == x; // flagged
//!   @override
//!   int get hashCode => x.hashCode; // flagged
//! }
//! ```
//!
//! The annotation may sit on the class itself or on any supertype. When a
//! supertype cannot be resolved the rule stays silent.
//!
//! Adopted from the official `avoid_equals_and_hash_code_on_mutable_classes`
//! lint.

use falcon_analyze::{AnalyzeContext, Rule, Truth};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct AvoidEqualsAndHashCodeOnMutableClasses;

impl Rule for AvoidEqualsAndHashCodeOnMutableClasses {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();
        for decl in &program.declarations {
            let (name, annotations, members, has_supertypes) = match decl {
                TopLevelDecl::Class(class) => (
                    &class.name.name,
                    &class.annotations,
                    &class.members,
                    class.extends.is_some()
                        || !class.with_clause.is_empty()
                        || !class.implements.is_empty(),
                ),
                TopLevelDecl::MixinClass(class) => (
                    &class.name.name,
                    &class.annotations,
                    &class.members,
                    class.extends.is_some()
                        || !class.with_clause.is_empty()
                        || !class.implements.is_empty(),
                ),
                _ => continue,
            };
            let immutable = match ctx.types {
                Some(types) => types.is_immutable_type(name),
                None if has_immutable_annotation(annotations, program) => Truth::Yes,
                None if !has_supertypes => Truth::No,
                None => Truth::Unknown,
            };
            if immutable != Truth::No {
                continue;
            }
            for member in members {
                let (member_name, annotations, span) = match member {
                    ClassMember::Operator(op) if op.op == "==" => ("==", &op.annotations, op.span),
                    ClassMember::Getter(getter) if getter.name.name == "hashCode" => {
                        ("hashCode", &getter.annotations, getter.span)
                    }
                    ClassMember::Method(method) if method.name.name == "hashCode" => {
                        ("hashCode", &method.annotations, method.span)
                    }
                    _ => continue,
                };
                let start = first_token_after(annotations, span, ctx.source);
                diags.push(ctx.diagnostic(
                    "avoid-equals-and-hash-code-on-mutable-classes",
                    format!(
                        "The method '{member_name}' should not be overridden in classes not annotated with '@immutable'."
                    ),
                    Span::new(start, span.end),
                ));
            }
        }
        diags
    }
}

/// The class's own package:meta `@immutable` annotation.
fn has_immutable_annotation(annotations: &[Annotation], program: &Program) -> bool {
    annotations.iter().any(|annotation| {
        let prefix = match annotation.name.as_slice() {
            [name] if name.name == "immutable" => None,
            [prefix, name] if name.name == "immutable" => Some(prefix.name.as_str()),
            _ => return false,
        };
        program.imports.iter().any(|import| {
            matches!(
                import.uri.value.as_str(),
                "package:meta/meta.dart"
                    | "package:flutter/foundation.dart"
                    | "package:flutter/widgets.dart"
                    | "package:flutter/material.dart"
            ) && import.as_name.as_ref().map(|name| name.name.as_str()) == prefix
        })
    })
}

/// Offset of the first source token after a member's annotations, skipping
/// whitespace and comments.
fn first_token_after(annotations: &[Annotation], span: Span, source: &str) -> usize {
    let mut offset = annotations
        .iter()
        .map(|annotation| annotation.span.end)
        .fold(span.start, usize::max);
    let bytes = source.as_bytes();
    while offset < span.end {
        let rest = &source[offset..];
        if bytes[offset].is_ascii_whitespace() {
            offset += 1;
        } else if rest.starts_with("//") {
            offset += rest.find('\n').unwrap_or(rest.len());
        } else if rest.starts_with("/*") {
            offset += rest.find("*/").map_or(rest.len(), |end| end + 2);
        } else {
            break;
        }
    }
    offset.min(span.end)
}
