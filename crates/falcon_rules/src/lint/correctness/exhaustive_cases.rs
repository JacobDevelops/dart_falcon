//! Require switch statements over enum-like classes to cover every constant value.

use std::collections::{HashMap, HashSet};

use falcon_analyze::{
    AnalyzeContext, ConstantValue, DeclarationFacts, DeclarationIdentity, ResolvedType, Rule,
    SemanticModel, SignatureIndex, TypeEnvironment, TypeParameterScope, evaluate_constant,
};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::Program;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr, walk_program, walk_stmt};

pub struct ExhaustiveCases;

impl Rule for ExhaustiveCases {
    fn name(&self) -> &'static str {
        "exhaustive-cases"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let (Some(identities), Some(signatures)) = (ctx.identities, ctx.signatures) else {
            return Vec::new();
        };
        let mut collector = Collector {
            diagnostics: Vec::new(),
            file: ctx.file_path.to_string_lossy().into_owned(),
            model: SemanticModel::new(ctx.file_path, identities, ctx.types),
            signatures,
            environment: TypeEnvironment::new(),
            type_parameters: TypeParameterScope::default(),
        };
        collector.visit_program(program);
        collector.diagnostics
    }
}

struct Collector<'a> {
    diagnostics: Vec<Diagnostic>,
    file: String,
    model: SemanticModel<'a>,
    signatures: &'a SignatureIndex,
    environment: TypeEnvironment,
    type_parameters: TypeParameterScope,
}

impl Collector<'_> {
    fn check_switch(&mut self, node: &SwitchStmt) {
        if node.cases.iter().any(|group| {
            group
                .cases
                .iter()
                .any(|case| matches!(case, SwitchCaseKind::Default))
        }) {
            return;
        }
        let ResolvedType::Interface { identity, .. } = self.environment.infer_with_signatures(
            &node.subject,
            &self.model,
            self.signatures,
            &self.type_parameters,
        ) else {
            return;
        };
        let Some(facts) = self.signatures.declaration(&identity) else {
            return;
        };
        if !is_enum_like(facts, &identity) {
            return;
        }
        let values = constant_values(facts, &identity, &self.model, self.signatures);
        if values.len() < 2 {
            return;
        }
        let mut covered = HashSet::new();
        for group in &node.cases {
            for case in &group.cases {
                let SwitchCaseKind::Pattern(pattern, _) = case else {
                    continue;
                };
                if let Some(name) = constant_pattern_name(pattern, &identity, &self.model)
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
            self.diagnostics.push(Diagnostic::new(
                "exhaustive-cases",
                Severity::Warning,
                format!(
                    "Add a case for '{}.{}'.",
                    identity_name(&identity),
                    preferred.name
                ),
                self.file.clone(),
                DiagSpan {
                    start: node.subject.span().start,
                    end: node.subject.span().end,
                },
            ));
        }
    }

    fn function(&mut self, params: &FormalParamList, body: Option<&FunctionBody>) {
        let saved = std::mem::replace(&mut self.environment, TypeEnvironment::new());
        self.environment
            .bind_params(params, &self.model, &self.type_parameters);
        if let Some(body) = body {
            match body {
                FunctionBody::Block(block) => {
                    for statement in &block.stmts {
                        visit_statement(self, statement);
                    }
                }
                FunctionBody::Arrow(expression, _) => self.visit_expr(expression),
                FunctionBody::Native(_, _) => {}
            }
        }
        self.environment = saved;
    }
}

impl Visitor for Collector<'_> {
    fn visit_program(&mut self, node: &Program) {
        walk_program(self, node);
    }

    fn visit_function_decl(&mut self, node: &FunctionDecl) {
        self.function(&node.params, node.body.as_ref());
    }

    fn visit_method_decl(&mut self, node: &MethodDecl) {
        self.function(&node.params, node.body.as_ref());
    }

    fn visit_constructor_decl(&mut self, node: &ConstructorDecl) {
        self.function(&node.params, node.body.as_ref());
    }

    fn visit_expr(&mut self, node: &Expr) {
        walk_expr(self, node);
    }

    fn visit_stmt(&mut self, node: &Stmt) {
        match node {
            Stmt::LocalVar(declaration) => {
                walk_stmt(self, node);
                let ty = declaration
                    .var_type
                    .as_ref()
                    .map(|ty| self.model.resolve_type_in(ty, &self.type_parameters))
                    .unwrap_or(ResolvedType::Unknown);
                for declarator in &declaration.declarators {
                    self.environment
                        .declare(declarator.name.name.clone(), ty.clone());
                }
            }
            Stmt::Block(block) => {
                self.environment.push_scope();
                for statement in &block.stmts {
                    visit_statement(self, statement);
                }
                self.environment.pop_scope();
            }
            Stmt::Switch(switch) => {
                self.check_switch(switch);
                walk_stmt(self, node);
            }
            Stmt::LocalFunc(_) => {}
            _ => walk_stmt(self, node),
        }
    }
}

fn visit_statement(visitor: &mut impl Visitor, statement: &Stmt) {
    visitor.visit_stmt(statement);
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
        .filter(|constant| exact_identity(&constant.ty, identity))
        .map(|constant| (constant.name.as_str(), &constant.initializer))
        .collect::<HashMap<_, _>>();
    fields
        .iter()
        .filter_map(|(name, expression)| {
            evaluate_constant(expression, identity, &fields, model, signatures)
                .map(|value| ((*name).to_string(), value))
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
    if constant.expr.is_some() || constant.name.is_empty() {
        return None;
    }
    let names = constant
        .name
        .iter()
        .map(|name| name.name.clone())
        .collect::<Vec<_>>();
    if names.len() == 1 {
        return names.into_iter().next();
    }
    let owner = model.resolve_name(&names[..names.len() - 1]);
    (owner.as_ref() == Some(identity))
        .then(|| names.last().cloned())
        .flatten()
}

fn identity_name(identity: &DeclarationIdentity) -> &str {
    match identity {
        DeclarationIdentity::Project { name, .. }
        | DeclarationIdentity::Sdk { name, .. }
        | DeclarationIdentity::Package { name, .. } => name,
    }
}
