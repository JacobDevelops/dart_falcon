//! Reuse or extend an existing record destructuring instead of rereading a field.
//! Records have immutable built-in fields. Object getters and list/map indexing
//! can execute user code or change value and are intentionally not candidates.

use std::collections::{HashMap, HashSet};

use falcon_analyze::resolve::lexical_usage::local_declarations;
use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{AnalyzeContext, ResolvedType, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use crate::lint::support::block_scope::same_block;

#[derive(Default)]
pub struct UseExistingDestructuring;

impl Rule for UseExistingDestructuring {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Collector {
            ctx,
            references: local_declarations(program),
            mutated: ctx.mutated_declarations(program),
            candidates: Vec::new(),
            diagnostics: Vec::new(),
        }))
    }
}

struct Candidate<'a> {
    declaration: PatternDeclaration<'a>,
    source: NodeId,
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    references: HashMap<usize, NodeId>,
    mutated: &'a HashSet<NodeId>,
    candidates: Vec<Candidate<'a>>,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn visit_stmt(&mut self, node: Stmt<'a>, state: &SemanticState<'a>) {
        let Stmt::PatternDecl(declaration) = node else {
            return;
        };
        let Expr::Ident(source) = declaration.init().unparenthesized() else {
            return;
        };
        let Some(&source_id) = self.references.get(&source.span().start) else {
            return;
        };
        if !self.mutated.contains(&source_id)
            && matches!(
                state.infer(declaration.init()),
                ResolvedType::Record {
                    nullable: false,
                    ..
                }
            )
            && matches!(declaration.pattern(), Pattern::Record(_))
        {
            self.candidates.push(Candidate {
                declaration,
                source: source_id,
            });
        }
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        let Expr::Field(field) = node else {
            return;
        };
        let Expr::Ident(source) = field.object().unparenthesized() else {
            return;
        };
        let Some(source_id) = self.references.get(&source.span().start) else {
            return;
        };
        let ResolvedType::Record {
            positional,
            named,
            nullable: false,
            ..
        } = state.infer(field.object())
        else {
            return;
        };
        let name = field.field().name();
        let index = name
            .strip_prefix('$')
            .and_then(|name| name.parse::<usize>().ok());
        if !named.iter().any(|(field, _)| field == name)
            && !index.is_some_and(|index| index > 0 && index <= positional.len())
        {
            return;
        }
        for candidate in &self.candidates {
            if *source_id != candidate.source
                || !same_block(candidate.declaration.syntax(), node.syntax())
            {
                continue;
            }
            let Pattern::Record(record) = candidate.declaration.pattern() else {
                continue;
            };
            let mut position = 0;
            let mut safe = true;
            for existing in record.fields() {
                let matching = if let Some(field_name) = existing.name() {
                    field_name.name() == name
                } else {
                    position += 1;
                    index == Some(position)
                };
                if matching {
                    safe = match existing.pattern() {
                        Pattern::Wildcard(_) => true,
                        Pattern::Variable(variable) => {
                            variable.type_().is_none()
                                && !self.mutated.contains(&variable.name().syntax().id())
                        }
                        _ => false,
                    };
                    break;
                }
            }
            if safe {
                self.diagnostics.push(self.ctx.diagnostic("Reuse or extend the existing record destructuring instead of reading this field again.", field.span()));
                break;
            }
        }
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }
}
