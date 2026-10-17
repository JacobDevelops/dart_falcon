//! Requires Flutter controllers owned by a `State` field to be disposed.

use std::collections::HashSet;

use crate::lint::lexical_usage::NameScopes;
use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, QualifiedName, ResolvedType, Rule, RulePreparation,
    SemanticModel, SignatureIndex, StructuralNode, StructuralRuleVisitor, StructuralTraversal,
    Truth,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct ProperControllerDispose;

impl Rule for ProperControllerDispose {
    fn prepare<'a>(
        &self,
        _program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let (Some(identities), Some(signatures)) = (ctx.identities, ctx.signatures) else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            model: SemanticModel::new(ctx.file_path, identities, ctx.types),
            signatures,
            diagnostics: Vec::new(),
            fields: HashSet::new(),
            owned: HashSet::new(),
            disposed: HashSet::new(),
            scopes: NameScopes::default(),
            in_dispose: false,
            active_owner: false,
            case_patterns: Vec::new(),
            if_conditions: Vec::new(),
            statement_header: false,
            headers: Vec::new(),
        }))
    }
}

fn is_flutter_state(name: &str, signatures: &SignatureIndex, model: &SemanticModel<'_>) -> bool {
    let Some(identity) = model.resolve_name(name) else {
        return false;
    };
    let ty = ResolvedType::interface_identity(identity);
    let state = DeclarationIdentity::flutter("State");
    signatures.is_subtype_of(&ty, &state, model) == Truth::Yes
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    model: SemanticModel<'a>,
    signatures: &'a SignatureIndex,
    diagnostics: Vec<Diagnostic>,
    fields: HashSet<String>,
    owned: HashSet<String>,
    disposed: HashSet<String>,
    scopes: NameScopes,
    in_dispose: bool,
    active_owner: bool,
    case_patterns: Vec<Span>,
    if_conditions: Vec<*const IfCondition>,
    statement_header: bool,
    headers: Vec<bool>,
}

impl Collector<'_> {
    fn field_target<'b>(&self, expression: &'b Expr) -> Option<&'b str> {
        match expression.unparenthesized() {
            Expr::Field { object, field, .. }
                if matches!(object.unparenthesized(), Expr::This { .. })
                    && self.fields.contains(&field.name) =>
            {
                Some(&field.name)
            }
            Expr::Ident(identifier)
                if self.fields.contains(&identifier.name)
                    && !self.scopes.bound(&identifier.name) =>
            {
                Some(&identifier.name)
            }
            _ => None,
        }
    }
}

falcon_analyze::structural_visitor! {
impl StructuralRuleVisitor for Collector<'_> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }

    fn visit_top_level_decl(&mut self, node: &TopLevelDecl) -> StructuralTraversal {
        self.active_owner = false;
        let (name, members) = match node {
            TopLevelDecl::Class(class) => (&class.name.name, &class.members),
            TopLevelDecl::MixinClass(class) => (&class.name.name, &class.members),
            _ => return StructuralTraversal::SkipChildren,
        };
        if !is_flutter_state(name, self.signatures, &self.model) {
            return StructuralTraversal::SkipChildren;
        }
        self.fields.clear();
        self.owned.clear();
        self.active_owner = true;
        self.disposed.clear();
        // Member names and initial ownership must be available before any body.
        for member in members {
            if let ClassMember::Field(field) = member {
                for declarator in &field.declarators {
                    if !field.is_static {
                        self.fields.insert(declarator.name.name.clone());
                    }
                    if declarator.initializer.as_ref().is_some_and(|initializer| {
                        constructs_controller(initializer, &self.model)
                    }) {
                        self.owned.insert(declarator.name.name.clone());
                    }
                }
            }
        }
        StructuralTraversal::Continue
    }

    fn exit_top_level_decl(&mut self, node: &TopLevelDecl) {
        let members = match node {
            TopLevelDecl::Class(class) => &class.members,
            TopLevelDecl::MixinClass(class) => &class.members,
            _ => return,
        };
        if !self.active_owner {
            return;
        }
        for member in members {
            if let ClassMember::Field(field) = member {
                for declarator in &field.declarators {
                    if self.owned.contains(&declarator.name.name)
                        && !self.disposed.contains(&declarator.name.name)
                    {
                        self.diagnostics.push(self.ctx.diagnostic("proper-controller-dispose", "This controller is never disposed; dispose it in the State's dispose() method.", declarator.name.span));
                    }
                }
            }
        }
    }

    fn visit_class_member(&mut self, node: &ClassMember) -> StructuralTraversal {
        let body = match node {
            ClassMember::Method(method) => method.body.as_ref(),
            ClassMember::Constructor(constructor) => constructor.body.as_ref(),
            ClassMember::Getter(getter) => getter.body.as_ref(),
            ClassMember::Setter(setter) => setter.body.as_ref(),
            ClassMember::Operator(operator) => operator.body.as_ref(),
            _ => None,
        };
        if body.is_none() {
            return StructuralTraversal::SkipChildren;
        }
        self.scopes.push();
        self.in_dispose = false;
        let params = match node {
            ClassMember::Method(method) => {
                self.in_dispose = method.name.name == "dispose";
                Some(&method.params)
            }
            ClassMember::Constructor(constructor) => Some(&constructor.params),
            ClassMember::Operator(operator) => Some(&operator.params),
            ClassMember::Setter(setter) => {
                self.scopes.declare(&setter.param.name);
                None
            }
            ClassMember::Getter(_) => None,
            _ => return StructuralTraversal::SkipChildren,
        };
        if let Some(params) = params {
            self.scopes.params(params);
        }
        StructuralTraversal::Continue
    }

    fn exit_class_member(&mut self, node: &ClassMember) {
        let has_body = match node {
            ClassMember::Method(method) => method.body.is_some(),
            ClassMember::Constructor(constructor) => constructor.body.is_some(),
            ClassMember::Getter(getter) => getter.body.is_some(),
            ClassMember::Setter(setter) => setter.body.is_some(),
            ClassMember::Operator(operator) => operator.body.is_some(),
            _ => false,
        };
        if has_body {
            self.scopes.pop();
        }
    }

    fn route_child(&mut self, parent: StructuralNode<'_>, child: StructuralNode<'_>) -> bool {
        match parent {
            StructuralNode::Program(_) => matches!(child, StructuralNode::TopLevelDecl(_)),
            StructuralNode::ClassDecl(_) | StructuralNode::MixinClass(_) => {
                matches!(child, StructuralNode::ClassMember(_))
            }
            StructuralNode::MethodDecl(_)
            | StructuralNode::ConstructorDecl(_)
            | StructuralNode::GetterDecl(_)
            | StructuralNode::SetterDecl(_)
            | StructuralNode::OperatorDecl(_)
            | StructuralNode::LocalFuncDecl(_)
            | StructuralNode::Expr(Expr::FuncExpr { .. }) => {
                matches!(child, StructuralNode::FunctionBody(_))
            }
            StructuralNode::CatchClause(_) => matches!(child, StructuralNode::Block(_)),
            StructuralNode::LocalVar(_) => matches!(child, StructuralNode::VarDeclarator(_)),
            StructuralNode::ForStmt(_) => {
                if matches!(child, StructuralNode::ForHeader(_)) {
                    self.statement_header = true;
                }
                true
            }
            StructuralNode::ForInit(_) if self.headers.last() == Some(&true) => {
                !matches!(child, StructuralNode::Annotation(_) | StructuralNode::DartType(_))
            }
            StructuralNode::PatternDeclaration(_) => {
                !matches!(child, StructuralNode::Annotation(_))
            }
            StructuralNode::IfCondition(condition @ IfCondition::Case(..))
                if self.if_conditions.last().copied() == Some(std::ptr::from_ref(condition)) =>
            {
                if let StructuralNode::Pattern(pattern) = child {
                    self.scopes.push();
                    self.case_patterns.push(*pattern.span());
                }
                true
            }
            StructuralNode::IfStmt(statement) => {
                if let StructuralNode::Stmt(branch) = child
                    && matches!(&statement.condition, IfCondition::Case(..))
                    && statement.else_branch.as_deref().is_some_and(|otherwise| {
                        std::ptr::eq(otherwise, branch)
                    })
                {
                    self.scopes.pop();
                }
                true
            }
            _ => true,
        }
    }

    fn visit_stmt(&mut self, node: &Stmt) -> StructuralTraversal {
        if let Stmt::If(statement) = node {
            self.if_conditions.push(std::ptr::from_ref(&statement.condition));
        }
        match node {
            Stmt::Block(_) | Stmt::For(_) => self.scopes.push(),
            Stmt::LocalFunc(function) => {
                self.scopes.declare(&function.name.name);
                self.scopes.push();
                self.scopes.params(&function.params);
            }
            _ => {}
        }
        StructuralTraversal::Continue
    }

    fn exit_stmt(&mut self, node: &Stmt) {
        if matches!(node, Stmt::If(_)) {
            self.if_conditions.pop();
        }
        if matches!(node, Stmt::Block(_) | Stmt::For(_) | Stmt::LocalFunc(_)) {
            self.scopes.pop();
        }
    }

    fn exit_if_stmt(&mut self, node: &IfStmt) {
        if matches!(&node.condition, IfCondition::Case(..)) && node.else_branch.is_none() {
            self.scopes.pop();
        }
    }

    fn visit_catch_clause(&mut self, node: &CatchClause) -> StructuralTraversal {
        self.scopes.push();
        if let Some(name) = &node.exception_var {
            self.scopes.declare(&name.name);
        }
        if let Some(name) = &node.stack_trace_var {
            self.scopes.declare(&name.name);
        }
        StructuralTraversal::Continue
    }

    fn exit_catch_clause(&mut self, _node: &CatchClause) {
        self.scopes.pop();
    }

    fn exit_var_declarator(&mut self, node: &VarDeclarator) {
        self.scopes.declare(&node.name.name);
    }

    fn exit_pattern(&mut self, node: &Pattern) {
        if self.case_patterns.last() == Some(node.span()) {
            self.case_patterns.pop();
            self.scopes.pattern(node);
        }
    }

    fn exit_pattern_declaration(&mut self, node: &PatternDeclaration) {
        self.scopes.pattern(&node.pattern);
    }

    fn visit_for_header(&mut self, _node: &ForHeader) -> StructuralTraversal {
        self.headers.push(std::mem::take(&mut self.statement_header));
        StructuralTraversal::Continue
    }

    fn exit_for_header(&mut self, _node: &ForHeader) {
        self.headers.pop();
    }

    fn exit_for_init(&mut self, node: &ForInit) {
        if self.headers.last() != Some(&true) {
            return;
        }
        match node {
            ForInit::ForIn { declares_variable: true, name, .. } => {
                self.scopes.declare(&name.name);
            }
            ForInit::PatternForIn { pattern, .. } => self.scopes.pattern(pattern),
            _ => {}
        }
    }

    fn visit_expr(&mut self, node: &Expr) -> StructuralTraversal {
        match node {
            Expr::Assign { target, value, .. }
                if constructs_controller(value, &self.model) =>
            {
                if let Some(field) = self.field_target(target) {
                    self.owned.insert(field.to_string());
                }
            }
            Expr::Call { callee, .. } if self.in_dispose => {
                if let Expr::Field { object, field, .. } = callee.unparenthesized()
                    && field.name == "dispose"
                    && let Some(controller) = self.field_target(object)
                {
                    self.disposed.insert(controller.to_string());
                }
            }
            Expr::Cascade { object, sections, .. }
                if self.in_dispose && sections.iter().any(|section| {
                    matches!(section.ops.first(), Some(CascadeOp::Call(name, _, _)) if name.name == "dispose")
                }) =>
            {
                if let Some(controller) = self.field_target(object) {
                    self.disposed.insert(controller.to_string());
                }
            }
            Expr::FuncExpr { params, .. } => {
                self.scopes.push();
                self.scopes.params(params);
            }
            _ => {}
        }
        StructuralTraversal::Continue
    }

    fn exit_expr(&mut self, node: &Expr) {
        if matches!(node, Expr::FuncExpr { .. }) {
            self.scopes.pop();
        }
    }
}
}

fn constructor_identity(callee: &Expr, model: &SemanticModel<'_>) -> Option<DeclarationIdentity> {
    match callee.unparenthesized() {
        Expr::Ident(identifier) => model.resolve_name(&identifier.name),
        Expr::Field { object, field, .. } => match object.unparenthesized() {
            Expr::Ident(identifier) => model
                .resolve_name(QualifiedName::prefixed(&identifier.name, &field.name))
                .or_else(|| model.resolve_name(&identifier.name)),
            Expr::Field {
                object: prefix,
                field: type_name,
                ..
            } => match prefix.unparenthesized() {
                Expr::Ident(prefix) => {
                    model.resolve_name(QualifiedName::prefixed(&prefix.name, &type_name.name))
                }
                _ => None,
            },
            _ => None,
        },
        _ => None,
    }
}

fn constructs_controller(expression: &Expr, model: &SemanticModel<'_>) -> bool {
    let identity = match expression.unparenthesized() {
        Expr::New { dart_type, .. } => match model.resolve_type(dart_type) {
            ResolvedType::Interface { identity, .. } => Some(identity),
            _ => None,
        },
        Expr::Call { callee, .. } => constructor_identity(callee, model),
        _ => None,
    };
    matches!(
        identity,
        Some(DeclarationIdentity::Package { package, name, .. })
            if package.as_str() == "flutter" && matches!(name.as_str(),
                "TextEditingController" | "ScrollController" | "PageController"
                | "TabController" | "AnimationController")
    )
}
