//! Ban declaration names, exempting references, initializing/super formals, and
//! `@override`-annotated members and their parameters.

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;
use glob::{MatchOptions, Pattern};
use serde_json::Value;

#[derive(Default)]
pub struct AvoidBannedNames;

pub(super) struct Entry<'a> {
    patterns: Vec<Pattern>,
    message: &'a str,
}

pub(super) fn entries<'a>(ctx: &'a AnalyzeContext<'a>, name: &str) -> Vec<Entry<'a>> {
    let Some(values) = crate::lint::options::for_rule(ctx, name)
        .and_then(|options| options.get("rules"))
        .and_then(Value::as_array)
    else {
        return Vec::new();
    };
    values
        .iter()
        .filter_map(|entry| {
            let message = entry.get("message")?.as_str()?;
            if message.trim().is_empty() {
                return None;
            }
            let values = entry.get("deny")?.as_array()?;
            if values.is_empty() {
                return None;
            }
            let patterns = values
                .iter()
                .map(|value| {
                    let value = value.as_str()?;
                    if value.is_empty() {
                        None
                    } else {
                        Pattern::new(value).ok()
                    }
                })
                .collect::<Option<Vec<_>>>()?;
            Some(Entry { patterns, message })
        })
        .collect()
}

pub(super) fn report(
    ctx: &AnalyzeContext<'_>,
    entries: &[Entry<'_>],
    value: &str,
    span: Span,
    diagnostics: &mut Vec<Diagnostic>,
) {
    const MATCH: MatchOptions = MatchOptions {
        case_sensitive: true,
        require_literal_separator: true,
        require_literal_leading_dot: false,
    };
    for entry in entries {
        if entry
            .patterns
            .iter()
            .any(|pattern| pattern.matches_with(value, MATCH))
        {
            diagnostics.push(ctx.diagnostic(entry.message, span));
        }
    }
}

impl Rule for AvoidBannedNames {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let entries = entries(ctx, "avoid-banned-names");
        if entries.is_empty() {
            return RulePreparation::Diagnostics(Vec::new());
        }
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            entries,
            diagnostics: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    entries: Vec<Entry<'a>>,
    diagnostics: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_constructor(&mut self, node: ConstructorRef<'a>) -> StructuralTraversal {
        if let Some(name) = node.constructor_name
            && !node.annotations.into_iter().any(|annotation| {
                annotation.name().any(|name| name.name() == "override")
            })
        {
            report(self.ctx, &self.entries, name.name(), name.span(), &mut self.diagnostics);
        }
        StructuralTraversal::Continue
    }

    fn visit_identifier(&mut self, node: Identifier<'a>) -> StructuralTraversal {
        let Some(parent) = node.syntax().parent() else {
            return StructuralTraversal::Continue;
        };
        if FormalParam::cast(parent).is_some_and(|parameter| parameter.initializing().is_some()) {
            return StructuralTraversal::Continue;
        }
        let additional_name = match parent.kind() {
            NodeKind::CatchClause | NodeKind::ImportDirective => true,
            NodeKind::ForInInit => ForInInit::cast(parent)
                .is_some_and(|init| init.declares_variable() && init.name().span() == node.span()),
            _ => false,
        };
        if !additional_name && (!matches!(parent.kind(),
            NodeKind::ClassDecl | NodeKind::MixinDecl | NodeKind::EnumDecl
            | NodeKind::ExtensionDecl | NodeKind::FunctionDecl | NodeKind::MethodDecl
            | NodeKind::GetterDecl | NodeKind::SetterDecl | NodeKind::VarDeclarator
            | NodeKind::FormalParam(_) | NodeKind::TypeParam | NodeKind::TypeAliasDecl
            | NodeKind::LocalFuncDecl | NodeKind::VariablePattern
            | NodeKind::ClassTypeAliasDecl | NodeKind::MixinClassDecl
            | NodeKind::ExtensionTypeDecl | NodeKind::EnumVariant
        ) || parent.children().find_map(Identifier::cast).is_none_or(|name| name.span() != node.span())) {
            return StructuralTraversal::Continue;
        }
        let declaration = if matches!(parent.kind(), NodeKind::FormalParam(_)) {
            parent.ancestors().skip(1).find(|ancestor| matches!(ancestor.kind(),
                NodeKind::MethodDecl | NodeKind::SetterDecl | NodeKind::FunctionDecl
                | NodeKind::ConstructorDecl | NodeKind::LocalFuncDecl | NodeKind::FuncExpr
            )).unwrap_or(parent)
        } else if parent.kind() == NodeKind::VarDeclarator {
            parent.parent().unwrap_or(parent)
        } else {
            parent
        };
        if declaration.children().filter_map(Annotation::cast)
            .any(|annotation| annotation.name().any(|name| name.name() == "override"))
        {
            return StructuralTraversal::Continue;
        }
        report(self.ctx, &self.entries, node.name(), node.span(), &mut self.diagnostics);
        StructuralTraversal::Continue
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }
}
}
