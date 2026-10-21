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

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation, resolve_call_constructor};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::visit::{
    Visitor, for_each_expr, walk_field_decl, walk_stmt, walk_top_level_decl,
};
use falcon_syntax::cst::*;
use falcon_syntax::token::TokenKind;

use crate::lint::const_eval::{ConstContexts, constant_in_context, deferred_prefixes};
use crate::lint::tokens::significant_tokens_in;
use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};

#[derive(Default)]
pub struct PreferConstDeclarations;

impl Rule for PreferConstDeclarations {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        if ctx.identities().is_none() {
            return RulePreparation::Diagnostics(Vec::new());
        }
        let mut declarations = Declarations::default();
        declarations.visit_program(program);
        let checker = Checker {
            ctx,
            declarations,
            contexts: ConstContexts::of(program),
            deferred: deferred_prefixes(program),
        };
        RulePreparation::Semantic(Box::new(checker))
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
        declarators: AstChildren<'_, VarDeclarator<'_>>,
        span: Span,
        is_local: bool,
    ) {
        if keyword != VarKeyword::Final {
            return;
        }
        let Some(last) = declarators.clone().last() else {
            return;
        };
        let index = self.groups.len();
        self.groups.push(Group {
            span: Span {
                start: span.start,
                end: last.span().end,
            },
            first_name: declarators.clone().next().unwrap().name().span().start,
            is_local,
            remaining: declarators.len(),
            valid: declarators
                .clone()
                .all(|declarator| declarator.initializer().is_some()),
        });
        for initializer in declarators.filter_map(|declarator| declarator.initializer()) {
            self.initializers.insert(initializer.span().start, index);
        }
    }
}

impl<'a> Visitor<'a> for Declarations {
    fn visit_top_level_decl(&mut self, node: TopLevelDecl<'a>) {
        if let TopLevelDecl::Variable(variable) = node {
            self.collect(
                variable.keyword(),
                variable.declarators(),
                variable.span(),
                false,
            );
        }
        walk_top_level_decl(self, node);
    }

    fn visit_field_decl(&mut self, node: FieldDecl<'a>) {
        if node.is_static() {
            self.collect(node.keyword(), node.declarators(), node.span(), false);
        }
        walk_field_decl(self, node);
    }

    fn visit_stmt(&mut self, node: Stmt<'a>) {
        if let Stmt::LocalVar(variable) = node {
            self.collect(
                variable.keyword(),
                variable.declarators(),
                variable.full_span(),
                true,
            );
        }
        walk_stmt(self, node);
    }
}

struct Checker<'a> {
    ctx: &'a AnalyzeContext<'a>,
    declarations: Declarations,
    contexts: ConstContexts,
    deferred: Vec<&'a str>,
}

impl<'a> SemanticRuleVisitor<'a> for Checker<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        let checker = *self;
        let ctx = checker.ctx;
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
                    let mut start = group.span.start;
                    let mut preceding_late = None;
                    for token in significant_tokens_in(
                        ctx.tree(),
                        ctx.source(),
                        Span::new(group.span.start, group.first_name),
                    ) {
                        if token.kind() == TokenKind::Final {
                            start = preceding_late.unwrap_or(token.offset());
                        }
                        preceding_late =
                            (token.kind() == TokenKind::Late).then_some(token.offset());
                    }
                    start
                };
                Some(ctx.diagnostic("Use 'const' for final variables initialized to a constant value.", Span {
                    start,
                    end: group.span.end,
                }))
            })
            .collect()
    }
    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        let Some(index) = self.declarations.initializers.remove(&node.span().start) else {
            return;
        };
        // The shared can-be-const checker also accepts expressions that would
        // become constant only after adding `const`. This rule requires an
        // already constant expression, not a fresh collection or instance.
        let mut already_constant = true;
        for_each_expr(node, &mut |expression| {
            if self.contexts.contains(&expression.span()) {
                return;
            }
            match expression {
                Expr::List(list) if !list.is_const() => already_constant = false,
                Expr::Map(map) if !map.is_const() => already_constant = false,
                Expr::Set(set) if !set.is_const() => already_constant = false,
                Expr::New(new) if !new.is_const() => already_constant = false,
                Expr::Call(call)
                    if !matches!(
                        call.callee().unparenthesized(),
                        Expr::DotShorthand(shorthand) if shorthand.is_const()
                    ) && resolve_call_constructor(call.callee(), &state.model, true)
                        .is_some() =>
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
    use falcon_analyze::RuleAnalysis;
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
        let parsed = parse(source);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let parsed = std::sync::Arc::new(falcon_analyze::ParsedFile {
            tree: parsed.tree,
            source: source.into(),
        });
        let program = parsed.tree.program(source);
        let identities = IdentityIndex::from_project_files(
            &[IdentitySource {
                path: &path,
                parsed: &parsed,
                has_parse_errors: false,
            }],
            &[],
        );
        let files = [(path.clone(), std::sync::Arc::clone(&parsed))];
        let types = TypeIndex::from_resolved_files(
            files.iter().map(|(path, parsed)| {
                (
                    path.as_path(),
                    falcon_analyze::LibrarySource {
                        program: parsed.program(),
                        has_parse_errors: false,
                        has_unresolved_parts: false,
                    },
                )
            }),
            &identities,
        );
        let signatures = SignatureIndex::from_project_files(&files, &identities, &types);
        let config = FalconConfig::default();
        let ctx = AnalyzeContext::new(&path, &config, &parsed)
            .with_identities(&identities)
            .with_types(&types)
            .with_signatures(&signatures);
        let diagnostics = PreferConstDeclarations.analyze(PreferConstDeclarations::NAME, program, &ctx);
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
