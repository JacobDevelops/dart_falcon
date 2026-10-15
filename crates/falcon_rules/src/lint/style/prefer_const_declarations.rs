//! Flags `final` declarations whose initializers are all constant expressions.
//!
//! A `final` local variable, top-level variable, or static field initialized
//! with a constant expression can be declared `const` instead. Plain collection
//! literals and constructor calls without `const` are not constant expressions.
//! Instance fields and for-loop initializer declarations are never reported,
//! and every declarator must qualify.
//! Diagnostic spans include local metadata and documentation, and include `late`
//! but exclude `static` on fields. They end at the last declarator, before
//! trailing trivia and the semicolon.
//!
//! Constant eligibility uses the shared lexical constant checker and evaluator,
//! including declaration-time local values, type parameters, deferred types,
//! and failed arithmetic, casts, or non-primitive equality. Analyzer's lint treats
//! generic type literals and constructor tear-offs as constants even with type
//! parameters or deferred type arguments. Abstract generative constructor
//! tear-offs have the same lint-only treatment. These parity quirks do not
//! guarantee that the suggested `const` declaration compiles.
//!
//! Adopted from the official `prefer_const_declarations` lint.

use std::collections::HashMap;

use falcon_analyze::{AnalyzeContext, Rule, resolve_call_constructor};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::token::TokenKind;
use falcon_syntax::visitor::{
    Visitor, for_each_expr, walk_field_decl, walk_stmt, walk_top_level_decl,
};

use crate::lint::const_eval::{ConstContexts, constant_in_context, deferred_prefixes};
use crate::lint::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};

#[derive(Default)]
pub struct PreferConstDeclarations;

impl Rule for PreferConstDeclarations {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(state) = SemanticState::new(program, ctx) else {
            return Vec::new();
        };
        let mut declarations = Declarations::default();
        declarations.visit_program(program);
        let mut checker = Checker {
            declarations,
            contexts: ConstContexts::of(program),
            deferred: deferred_prefixes(program),
        };
        visit_program(&mut checker, program, state);
        checker
            .declarations
            .groups
            .into_iter()
            .filter_map(|group| {
                if !group.valid || group.remaining != 0 {
                    return None;
                }
                let start = if group.is_local {
                    group.span.start
                } else {
                    let tokens = ctx.tokens();
                    let from = tokens.partition_point(|token| token.offset < group.span.start);
                    let until = tokens.partition_point(|token| token.offset < group.first_name);
                    let mut prefix = tokens[from..until]
                        .iter()
                        .rev()
                        .filter(|token| !token.is_trivia());
                    prefix.find(|token| token.kind == TokenKind::Final).map_or(
                        group.span.start,
                        |final_keyword| {
                            prefix
                                .next()
                                .filter(|token| token.kind == TokenKind::Late)
                                .unwrap_or(final_keyword)
                                .offset
                        },
                    )
                };
                Some(ctx.diagnostic(
                    "prefer-const-declarations",
                    "Use 'const' for final variables initialized to a constant value.",
                    Span {
                        start,
                        end: group.span.end,
                    },
                ))
            })
            .collect()
    }
}

struct Group {
    span: Span,
    first_name: usize,
    is_local: bool,
    remaining: usize,
    valid: bool,
}

#[derive(Default)]
struct Declarations {
    groups: Vec<Group>,
    initializers: HashMap<usize, usize>,
}

impl Declarations {
    fn collect(
        &mut self,
        keyword: VarKeyword,
        declarators: &[VarDeclarator],
        span: Span,
        is_local: bool,
    ) {
        if keyword != VarKeyword::Final {
            return;
        }
        let Some(last) = declarators.last() else {
            return;
        };
        let index = self.groups.len();
        self.groups.push(Group {
            span: Span {
                start: span.start,
                end: last.span.end,
            },
            first_name: declarators[0].name.span.start,
            is_local,
            remaining: declarators.len(),
            valid: declarators
                .iter()
                .all(|declarator| declarator.initializer.is_some()),
        });
        for initializer in declarators
            .iter()
            .filter_map(|declarator| declarator.initializer.as_ref())
        {
            self.initializers.insert(initializer.span().start, index);
        }
    }
}

impl Visitor for Declarations {
    fn visit_top_level_decl(&mut self, node: &TopLevelDecl) {
        if let TopLevelDecl::Variable(variable) = node {
            self.collect(
                variable.keyword,
                &variable.declarators,
                variable.span,
                false,
            );
        }
        walk_top_level_decl(self, node);
    }

    fn visit_field_decl(&mut self, node: &FieldDecl) {
        if node.is_static {
            self.collect(node.keyword, &node.declarators, node.span, false);
        }
        walk_field_decl(self, node);
    }

    fn visit_stmt(&mut self, node: &Stmt) {
        if let Stmt::LocalVar(variable) = node {
            self.collect(variable.keyword, &variable.declarators, variable.span, true);
        }
        walk_stmt(self, node);
    }
}

struct Checker<'a> {
    declarations: Declarations,
    contexts: ConstContexts,
    deferred: Vec<&'a str>,
}

impl SemanticRuleVisitor for Checker<'_> {
    fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
        let Some(index) = self.declarations.initializers.remove(&node.span().start) else {
            return;
        };
        // The shared can-be-const checker also accepts expressions that would
        // become constant only after adding `const`. This rule requires an
        // already constant expression, not a fresh collection or instance.
        let mut already_constant = true;
        for_each_expr(node, &mut |expression| {
            if self.contexts.contains(expression.span()) {
                return;
            }
            match expression {
                Expr::List {
                    is_const: false, ..
                }
                | Expr::Map {
                    is_const: false, ..
                }
                | Expr::Set {
                    is_const: false, ..
                }
                | Expr::New {
                    is_const: false, ..
                } => already_constant = false,
                Expr::Call { callee, .. }
                    if !matches!(
                        callee.unparenthesized(),
                        Expr::DotShorthand { is_const: true, .. }
                    ) && resolve_call_constructor(callee, &state.model, true).is_some() =>
                {
                    already_constant = false;
                }
                _ => {}
            }
        });
        let group = &mut self.declarations.groups[index];
        group.remaining -= 1;
        group.valid &= already_constant && constant_in_context(node, &self.deferred, state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use falcon_analyze::{IdentityIndex, IdentitySource, SignatureIndex, TypeIndex};
    use falcon_config::FalconConfig;
    use falcon_dart_parser::parse;
    use std::path::PathBuf;

    #[test]
    fn declaration_spans_preserve_modifiers_and_exclude_semicolons_and_trivia() {
        let source = r#"
@deprecated
late /* prefix */ final top = 1 /* interior */, other = (2) /* trailing */ ;
class C {
  @deprecated
  static /* excluded */ late /* included */ final field = 3 /* trailing */ ;
  static final plain = 4 /* trailing */ ;
}
void f() {
  @deprecated
  late /* included */ final local = 5 /* trailing */ ;
  /// Local documentation.
  @deprecated
  late final documented = 6 /* trailing */ ;
}
"#;
        let path = PathBuf::from("/project/lib/test.dart");
        let (program, errors) = parse(source);
        assert!(errors.is_empty(), "{errors:?}");
        let identities = IdentityIndex::from_project_files(
            &[IdentitySource {
                path: &path,
                program: &program,
                has_parse_errors: false,
            }],
            &[],
        );
        let files = [(path.clone(), &program)];
        let types = TypeIndex::from_resolved_files(
            files.iter().map(|(path, program)| {
                (
                    path.as_path(),
                    falcon_analyze::LibrarySource {
                        program,
                        has_parse_errors: false,
                        has_unresolved_parts: false,
                    },
                )
            }),
            &identities,
        );
        let signatures = SignatureIndex::from_project_files(&files, &identities, &types);
        let config = FalconConfig::default();
        let ctx = AnalyzeContext::new(&path, source, &config)
            .with_identities(&identities)
            .with_types(&types)
            .with_signatures(&signatures);
        let diagnostics = PreferConstDeclarations.analyze(&program, &ctx);
        let spans: Vec<_> = diagnostics
            .iter()
            .map(|diagnostic| &source[diagnostic.span.start..diagnostic.span.end])
            .collect();
        assert_eq!(
            spans,
            [
                "late /* prefix */ final top = 1 /* interior */, other = (2)",
                "late /* included */ final field = 3",
                "final plain = 4",
                "@deprecated\n  late /* included */ final local = 5",
                "/// Local documentation.\n  @deprecated\n  late final documented = 6",
            ]
        );
    }
}
