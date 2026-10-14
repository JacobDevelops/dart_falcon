//! Require switch statements over enum-like classes to cover every constant value.

use std::collections::{HashMap, HashSet};

use crate::lint::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};
use falcon_analyze::{
    AnalyzeContext, ConstantValue, DeclarationFacts, DeclarationIdentity, ResolvedType, Rule,
    SemanticModel, SignatureIndex, evaluate_constant,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;
use falcon_syntax::ast::*;

pub struct ExhaustiveCases;

impl Rule for ExhaustiveCases {
    fn name(&self) -> &'static str {
        "exhaustive-cases"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(state) = SemanticState::new(program, ctx) else {
            return Vec::new();
        };
        let mut collector = Collector {
            diagnostics: Vec::new(),
            ctx,
        };
        visit_program(&mut collector, program, state);
        collector.diagnostics
    }
}

struct Collector<'a> {
    diagnostics: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
}

impl Collector<'_> {
    fn check_switch(&mut self, node: &SwitchStmt, state: &SemanticState<'_>) {
        if node.cases.iter().any(|group| {
            group
                .cases
                .iter()
                .any(|case| matches!(case, SwitchCaseKind::Default))
        }) {
            return;
        }
        let ResolvedType::Interface { identity, .. } = state.infer(&node.subject) else {
            return;
        };
        let Some(facts) = state.signatures.declaration(&identity) else {
            return;
        };
        if !is_enum_like(facts, &identity) {
            return;
        }
        let values = constant_values(facts, &identity, &state.model, &state.signatures);
        if values.len() < 2 {
            return;
        }
        let mut covered = HashSet::new();
        for group in &node.cases {
            for case in &group.cases {
                let SwitchCaseKind::Pattern(pattern, _) = case else {
                    continue;
                };
                if let Some(name) = constant_pattern_name(pattern, &identity, &state.model)
                    && let Some(value) = values.get(&name)
                {
                    covered.insert(value.clone());
                }
            }
        }
        let mut groups: HashMap<ConstantValue, Vec<_>> = HashMap::new();
        for constant in &facts.static_consts {
            if let Some(value) = values.get(&constant.name) {
                groups.entry(value.clone()).or_default().push(constant);
            }
        }
        for (value, aliases) in groups {
            if covered.contains(&value) {
                continue;
            }
            let preferred = aliases
                .iter()
                .find(|constant| !constant.is_deprecated)
                .copied()
                .unwrap_or(aliases[0]);
            self.diagnostics.push(self.ctx.diagnostic(
                "exhaustive-cases",
                format!("Add a case for '{}.{}'.", identity.name(), preferred.name),
                *node.subject.span(),
            ));
        }
    }
}

impl SemanticRuleVisitor for Collector<'_> {
    fn visit_stmt(&mut self, node: &Stmt, state: &SemanticState<'_>) {
        if let Stmt::Switch(switch) = node {
            self.check_switch(switch, state);
        }
    }
}

fn is_enum_like(facts: &DeclarationFacts, identity: &DeclarationIdentity) -> bool {
    !facts.is_abstract
        && !facts.has_subclass_in_library
        && !facts.constructors.is_empty()
        && facts
            .constructors
            .iter()
            .all(|constructor| constructor.is_private && !constructor.is_factory)
        && facts
            .static_consts
            .iter()
            .filter(|constant| exact_identity(&constant.ty, identity))
            .count()
            >= 2
}

fn exact_identity(ty: &ResolvedType, expected: &DeclarationIdentity) -> bool {
    matches!(ty, ResolvedType::Interface { identity, arguments, nullable: false, .. }
        if identity == expected && arguments.is_empty())
}

fn constant_values(
    facts: &DeclarationFacts,
    identity: &DeclarationIdentity,
    model: &SemanticModel<'_>,
    signatures: &SignatureIndex,
) -> HashMap<String, ConstantValue> {
    let fields = facts
        .static_consts
        .iter()
        .map(|constant| (constant.name.as_str(), &constant.initializer))
        .collect::<HashMap<_, _>>();
    facts
        .static_consts
        .iter()
        .filter(|constant| exact_identity(&constant.ty, identity))
        .filter_map(|constant| {
            evaluate_constant(
                &constant.initializer,
                Some(identity),
                &fields,
                model,
                signatures,
            )
            .map(|value| (constant.name.clone(), value))
        })
        .collect()
}

fn constant_pattern_name(
    pattern: &Pattern,
    identity: &DeclarationIdentity,
    model: &SemanticModel<'_>,
) -> Option<String> {
    let Pattern::Const(constant) = pattern else {
        return None;
    };
    let ConstPatternValue::Name(name) = &constant.value else {
        return None;
    };
    if name.is_empty() {
        return None;
    }
    let names = name
        .iter()
        .map(|name| name.name.clone())
        .collect::<Vec<_>>();
    if names.len() == 1 {
        return names.into_iter().next();
    }
    let owner = model.resolve_dotted(&names[..names.len() - 1]);
    (owner.as_ref() == Some(identity))
        .then(|| names.last().cloned())
        .flatten()
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use falcon_analyze::{IdentityIndex, IdentitySource, TypeIndex};
    use falcon_config::FalconConfig;
    use falcon_dart_parser::parse;

    use super::*;

    #[test]
    fn single_file_fallback_preserves_owner_constant_dependencies() {
        let source = include_str!("../../../tests/corpus/exhaustive-cases/owner-dependencies.dart");
        let (program, errors) = parse(source);
        assert!(errors.is_empty(), "{errors:?}");
        let path = Path::new("owner-dependencies.dart");
        let identities = IdentityIndex::from_project_files(
            &[IdentitySource {
                path,
                program: &program,
                has_parse_errors: false,
            }],
            &[],
        );
        let types = TypeIndex::from_program(&program);
        let config = FalconConfig::default();
        let ctx = AnalyzeContext::new(path, source, &config)
            .with_identities(&identities)
            .with_types(&types);
        let diagnostics = ExhaustiveCases.analyze(&program, &ctx);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].message, "Add a case for 'Token.second'.");
    }
}
