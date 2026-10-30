//! Prefer one resolved Riverpod notifier class per file.
//!
//! Includes synchronous, asynchronous, stream, auto-dispose, family and state
//! notifiers, as well as indirect subclasses and private notifier classes.
//! Unrelated classes with notifier-like names do not count.

use crate::lint::riverpod::{NOTIFIERS, is_type};
use falcon_analyze::{AnalyzeContext, Rule, RulePreparation, SemanticRuleVisitor, SemanticState};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct PreferSingleNotifierPerFile;
impl Rule for PreferSingleNotifierPerFile {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Collector {
            program,
            ctx,
            diagnostics: Vec::new(),
        }))
    }
}
struct Collector<'a> {
    program: Program<'a>,
    ctx: &'a AnalyzeContext<'a>,
    diagnostics: Vec<Diagnostic>,
}
impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn visit_program_end(&mut self, state: &SemanticState<'a>) {
        let mut seen = false;
        for declaration in self.program.declarations() {
            let name = match declaration {
                TopLevelDecl::Class(node) => node.name(),
                TopLevelDecl::MixinClass(node) => node.name(),
                _ => continue,
            };
            if let Some(ty) = state.model.resolve_type_name(&[name.name()])
                && is_type(self.ctx, state, &ty, NOTIFIERS)
            {
                if seen {
                    self.diagnostics.push(
                        self.ctx
                            .diagnostic("Prefer only one notifier per file.", name.span()),
                    );
                }
                seen = true;
            }
        }
    }
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }
}
