//! Flags explicit `= null` initializers on declarations that already default to null.
//!
//! A non-`final`, non-`const`, non-`late` variable whose type is nullable,
//! absent, or `dynamic` is already initialized to `null`, so writing `= null`
//! adds noise without changing behavior. Drop the initializer and let the
//! default stand. Declarations that are `final`, `const`, `late`, or have a
//! non-nullable type are left alone, since for them the `null` is either
//! required or meaningful.

use falcon_analyze::{
    AnalyzeContext, ParameterDefault, ResolvedType, Rule, RulePreparation, StructuralRuleVisitor,
    StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

use falcon_analyze::semantic_scope::SemanticState;

#[derive(Default)]
pub struct AvoidInitToNull;

impl Rule for AvoidInitToNull {
    fn prepare<'a>(
        &self,
        program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let state = SemanticState::new(program, ctx);
        let collector = Collector {
            diags: Vec::new(),
            ctx,
            state,
            superclass: None,
        };
        RulePreparation::Structural(Box::new(collector))
    }
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
    state: Option<SemanticState<'a>>,
    superclass: Option<ResolvedType>,
}

impl Collector<'_> {
    /// A declaration only defaults to null when it is neither `final`, `const`
    /// nor `late` and its type is nullable (or absent/`dynamic`). In that case
    /// the `= null` is redundant and can be dropped without changing behaviour.
    fn check(
        &mut self,
        keyword: VarKeyword,
        is_late: bool,
        var_type: &Option<DartType>,
        declarators: &[VarDeclarator],
    ) {
        if keyword != VarKeyword::Var || is_late || !defaults_to_null(var_type) {
            return;
        }
        for d in declarators {
            if matches!(
                d.initializer.as_ref().map(Expr::unparenthesized),
                Some(Expr::NullLit { .. })
            ) {
                self.diags.push(self.ctx.diagnostic(
                    "avoid-init-to-null",
                    "Don't explicitly initialize variables to null.",
                    d.span,
                ));
            }
        }
    }

    fn check_super_defaults(
        &mut self,
        params: &FormalParamList,
        initializers: &[ConstructorInitializer],
    ) {
        let (Some(state), Some(ResolvedType::Interface { identity, .. })) =
            (self.state.as_ref(), self.superclass.as_ref())
        else {
            return;
        };
        let target = initializers
            .iter()
            .find_map(|initializer| match initializer {
                ConstructorInitializer::SuperCall { call_name, .. } => {
                    Some(call_name.as_ref().map_or("new", |name| name.name.as_str()))
                }
                _ => None,
            })
            .unwrap_or("new");
        let Some(signature) = state.signatures.constructor(identity, target) else {
            return;
        };
        let mut position = 0;
        for param in &params.params {
            if param.initializing != Some(InitializingFormal::Super) {
                continue;
            }
            let name = if matches!(param.kind, ParamKind::Named { .. }) {
                Some(param.name.name.as_str())
            } else {
                let name = signature
                    .positional_parameter_names
                    .get(position)
                    .map(String::as_str);
                position += 1;
                name
            };
            let exists = if matches!(param.kind, ParamKind::Named { .. }) {
                signature.named.contains_key(&param.name.name)
            } else {
                position <= signature.positional.len()
            };
            if exists
                && name
                    .and_then(|name| signature.defaults.get(name))
                    .is_none_or(default_is_null)
            {
                Self::check_default(self.ctx, &mut self.diags, param);
            }
        }
    }

    fn check_default(ctx: &AnalyzeContext<'_>, diags: &mut Vec<Diagnostic>, param: &FormalParam) {
        if defaults_to_null(&param.param_type)
            && matches!(
                param.default_value.as_ref().map(Expr::unparenthesized),
                Some(Expr::NullLit { .. })
            )
        {
            diags.push(ctx.diagnostic(
                "avoid-init-to-null",
                "Don't explicitly initialize variables to null.",
                param.span,
            ));
        }
    }
}

fn defaults_to_null(var_type: &Option<DartType>) -> bool {
    match var_type {
        None | Some(DartType::Dynamic { .. }) => true,
        Some(t) => t.is_nullable(),
    }
}

fn default_is_null(default: &ParameterDefault) -> bool {
    match default {
        ParameterDefault::Null => true,
        ParameterDefault::Declared(expression) => {
            matches!(expression.as_ref(), Expr::NullLit { .. })
        }
        ParameterDefault::Inherited { default, .. } => default_is_null(default),
    }
}

falcon_analyze::structural_visitor! {
impl StructuralRuleVisitor for Collector<'_> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_top_level_decl(&mut self, node: &TopLevelDecl) -> StructuralTraversal {
        self.superclass = match node {
            TopLevelDecl::Class(class) => class.extends.as_ref(),
            TopLevelDecl::MixinClass(class) => class.extends.as_ref(),
            _ => None,
        }
        .and_then(|ty| self.state.as_ref().map(|state| state.model.resolve_type(ty)));
        if let TopLevelDecl::Class(class) = node
            && let Some(constructor) = &class.primary_constructor
        {
            let initializers = class
                .members
                .iter()
                .find_map(|member| match member {
                    ClassMember::PrimaryConstructorBody(body) => Some(body.initializers.as_slice()),
                    _ => None,
                })
                .unwrap_or(&[]);
            self.check_super_defaults(&constructor.params, initializers);
        }
        if let TopLevelDecl::Variable(v) = node {
            self.check(v.keyword, v.is_late, &v.var_type, &v.declarators);
        }
        StructuralTraversal::Continue
    }

    fn visit_field_decl(&mut self, node: &FieldDecl) -> StructuralTraversal {
        self.check(
            node.keyword,
            node.is_late,
            &node.field_type,
            &node.declarators,
        );
        StructuralTraversal::Continue
    }

    fn visit_local_var(&mut self, lv: &LocalVarDecl) -> StructuralTraversal {
        self.check(lv.keyword, lv.is_late, &lv.var_type, &lv.declarators);
        StructuralTraversal::Continue
    }

    fn visit_formal_param(&mut self, param: &FormalParam) -> StructuralTraversal {
        if param.initializing != Some(InitializingFormal::Super) {
            Self::check_default(self.ctx, &mut self.diags, param);
        }
        StructuralTraversal::Continue
    }

    fn visit_class_member(&mut self, member: &ClassMember) -> StructuralTraversal {
        if let ClassMember::Constructor(constructor) = member {
            self.check_super_defaults(&constructor.params, &constructor.initializers);
        }
        StructuralTraversal::Continue
    }
}
}
