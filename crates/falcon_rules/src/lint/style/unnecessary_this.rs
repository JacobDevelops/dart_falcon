//! Flags a redundant `this.` qualifier on member access.
//!
//! Lexical scopes are prebound before checking instance member bodies.

use crate::lint::lexical_usage::NameScopes;
use std::collections::HashSet;

use falcon_analyze::{
    AnalyzeContext, DocName, Rule, RulePreparation, SemanticModel, StructuralNode,
    StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;

#[derive(Default)]
pub struct UnnecessaryThis;

impl Rule for UnnecessaryThis {
    fn prepare<'a>(
        &self,
        program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            program,
            model: ctx
                .identities
                .map(|identities| SemanticModel::new(ctx.file_path, identities, ctx.types)),
            diagnostics: Vec::new(),
            names: HashSet::new(),
            scopes: NameScopes::default(),
            statement_scopes: Vec::new(),
            block_scopes: Vec::new(),
            scoped_statement: false,
            scoped_block: false,
            for_inits: Vec::new(),
            if_conditions: Vec::new(),
            binding_patterns: Vec::new(),
            pattern_declarations: Vec::new(),
        }))
    }
}

fn library_contains_name(program: &Program, model: Option<&SemanticModel<'_>>, name: &str) -> bool {
    if program.imports.iter().any(|import| {
        import
            .as_name
            .as_ref()
            .is_some_and(|prefix| prefix.name == name)
    }) {
        return true;
    }
    if let Some(model) = model {
        return matches!(
            model.resolve_library_name(name),
            DocName::Declaration { .. } | DocName::Prefix | DocName::Extension
        );
    }
    program
        .declarations
        .iter()
        .any(|declaration| match declaration {
            TopLevelDecl::Function(function) => function.name.name == name,
            TopLevelDecl::Variable(variable) => variable
                .declarators
                .iter()
                .any(|declarator| declarator.name.name == name),
            _ => false,
        })
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    program: &'a Program,
    model: Option<SemanticModel<'a>>,
    diagnostics: Vec<Diagnostic>,
    names: HashSet<String>,
    scopes: NameScopes,
    statement_scopes: Vec<usize>,
    block_scopes: Vec<bool>,
    scoped_statement: bool,
    scoped_block: bool,
    for_inits: Vec<Option<*const ForInit>>,
    if_conditions: Vec<*const IfCondition>,
    binding_patterns: Vec<*const Pattern>,
    pattern_declarations: Vec<(usize, usize)>,
}

impl Collector<'_> {
    fn prebind(&mut self, statements: &[Stmt]) {
        for statement in statements {
            match statement {
                Stmt::LocalVar(declaration) => {
                    for declarator in &declaration.declarators {
                        self.scopes.declare(&declarator.name.name);
                    }
                }
                Stmt::LocalFunc(function) => self.scopes.declare(&function.name.name),
                Stmt::PatternDecl(declaration) => self.scopes.pattern(&declaration.pattern),
                _ => {}
            }
        }
    }

    fn statement_for_init(&self, node: &ForInit) -> bool {
        self.for_inits.last().copied().flatten() == Some(std::ptr::from_ref(node))
    }
}

falcon_analyze::structural_visitor! {
impl StructuralRuleVisitor for Collector<'_> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }

    fn visit_top_level_decl(&mut self, node: &TopLevelDecl) -> StructuralTraversal {
        let Some(members) = crate::lint::declarations::members_of(node) else {
            return StructuralTraversal::SkipChildren;
        };
        self.names = crate::lint::declarations::member_names(members);
        let effective = match node {
            TopLevelDecl::Class(node) => node.element_members(),
            TopLevelDecl::MixinClass(node) => node.element_members(),
            TopLevelDecl::Enum(node) => node.element_members(),
            TopLevelDecl::ExtensionType(node) => node.element_members(),
            _ => EffectiveMembers::declared(members),
        };
        self.names.extend(
            effective.declaring_params().map(|param| param.name.name.clone()),
        );
        StructuralTraversal::Continue
    }

    fn visit_class_member(&mut self, node: &ClassMember) -> StructuralTraversal {
        let (params, setter_param) = match node {
            ClassMember::Method(method) if !method.is_static => (Some(&method.params), None),
            ClassMember::Getter(getter) if !getter.is_static => (None, None),
            ClassMember::Setter(setter) if !setter.is_static => (None, Some(&setter.param.name)),
            ClassMember::Operator(operator) => (Some(&operator.params), None),
            ClassMember::Constructor(constructor) => (Some(&constructor.params), None),
            _ => return StructuralTraversal::SkipChildren,
        };
        self.scopes.push();
        if let Some(params) = params {
            self.scopes.params(params);
        }
        if let Some(name) = setter_param {
            self.scopes.declare(name);
        }
        StructuralTraversal::Continue
    }

    fn exit_class_member(&mut self, node: &ClassMember) {
        let scanned = match node {
            ClassMember::Method(method) => !method.is_static,
            ClassMember::Getter(getter) => !getter.is_static,
            ClassMember::Setter(setter) => !setter.is_static,
            ClassMember::Operator(_) | ClassMember::Constructor(_) => true,
            _ => false,
        };
        if scanned {
            self.scopes.pop();
        }
    }

    fn route_child(&mut self, parent: StructuralNode<'_>, child: StructuralNode<'_>) -> bool {
        match parent {
            StructuralNode::Program(_) => matches!(child, StructuralNode::TopLevelDecl(_)),
            StructuralNode::ClassDecl(_) | StructuralNode::MixinClass(_)
            | StructuralNode::MixinDecl(_) | StructuralNode::EnumDecl(_)
            | StructuralNode::ExtensionDecl(_) | StructuralNode::ExtensionType(_) => {
                matches!(child, StructuralNode::ClassMember(_))
            }
            // Only bodies need qualifier checks; defaults, metadata and
            // constructor initializers have separate name-resolution rules.
            StructuralNode::MethodDecl(_) | StructuralNode::GetterDecl(_)
            | StructuralNode::SetterDecl(_) | StructuralNode::OperatorDecl(_)
            | StructuralNode::ConstructorDecl(_) | StructuralNode::LocalFuncDecl(_) => {
                matches!(child, StructuralNode::FunctionBody(_))
            }
            StructuralNode::Expr(Expr::FuncExpr { .. }) => {
                matches!(child, StructuralNode::FunctionBody(_))
            }
            StructuralNode::LocalVar(_) => matches!(child, StructuralNode::VarDeclarator(_)),
            StructuralNode::PatternDeclaration(_) => {
                if matches!(child, StructuralNode::Expr(_))
                    && let Some((_, split)) = self.pattern_declarations.last_mut()
                {
                    *split = self.diagnostics.len();
                }
                matches!(child, StructuralNode::Pattern(_) | StructuralNode::Expr(_))
            }
            StructuralNode::ForInit(init) if self.statement_for_init(init) => {
                if let StructuralNode::Pattern(pattern) = child {
                    self.binding_patterns.push(std::ptr::from_ref(pattern));
                }
                matches!(child, StructuralNode::LocalVar(_) | StructuralNode::Pattern(_)
                    | StructuralNode::Expr(_))
            }
            StructuralNode::IfCondition(condition @ IfCondition::Case(..))
                if self.if_conditions.last().copied() == Some(std::ptr::from_ref(condition)) =>
            {
                if let StructuralNode::Pattern(pattern) = child {
                    self.scopes.push();
                    self.binding_patterns.push(std::ptr::from_ref(pattern));
                }
                true
            }
            StructuralNode::IfStmt(statement) => {
                if let StructuralNode::Stmt(branch) = child {
                    // A case's pattern scope starts after its value and remains
                    // live through the guard and then branch, but not the else.
                    if !std::ptr::eq(branch, statement.then_branch.as_ref())
                        || !matches!(statement.condition, IfCondition::Case(..))
                    {
                        self.scopes.push();
                    }
                    self.scoped_statement = true;
                }
                true
            }
            StructuralNode::WhileStmt(_) => {
                if matches!(child, StructuralNode::Stmt(_)) {
                    self.scopes.push();
                    self.scoped_statement = true;
                }
                true
            }
            StructuralNode::TryCatchStmt(_) => {
                self.scoped_block = matches!(child, StructuralNode::Block(_));
                true
            }
            StructuralNode::CatchClause(_) => matches!(child, StructuralNode::Block(_)),
            StructuralNode::SwitchCaseKind(SwitchCaseKind::Pattern(..)) => {
                if let StructuralNode::Pattern(pattern) = child {
                    self.binding_patterns.push(std::ptr::from_ref(pattern));
                }
                true
            }
            _ => true,
        }
    }

    fn visit_stmt(&mut self, node: &Stmt) -> StructuralTraversal {
        let mut scopes = usize::from(std::mem::take(&mut self.scoped_statement));
        if matches!(node, Stmt::Block(_) | Stmt::For(_) | Stmt::DoWhile(_)) {
            self.scopes.push();
            scopes += 1;
        }
        self.statement_scopes.push(scopes);
        if let Stmt::For(statement) = node {
            self.for_inits.push(statement.header.init.as_ref().map(std::ptr::from_ref));
        }
        if let Stmt::If(statement) = node {
            self.if_conditions.push(std::ptr::from_ref(&statement.condition));
        }
        StructuralTraversal::Continue
    }

    fn exit_stmt(&mut self, node: &Stmt) {
        if matches!(node, Stmt::For(_)) {
            self.for_inits.pop();
        }
        if matches!(node, Stmt::If(_)) {
            self.if_conditions.pop();
        }
        for _ in 0..self.statement_scopes.pop().expect("statement scope") {
            self.scopes.pop();
        }
    }

    fn visit_block(&mut self, node: &Block) -> StructuralTraversal {
        let scoped = std::mem::take(&mut self.scoped_block);
        if scoped {
            self.scopes.push();
        }
        self.block_scopes.push(scoped);
        // Dart prebinds all block declarations, including later declarations
        // and names referenced by their own initializers.
        self.prebind(&node.stmts);
        StructuralTraversal::Continue
    }

    fn exit_block(&mut self, _node: &Block) {
        if self.block_scopes.pop().expect("block scope") {
            self.scopes.pop();
        }
    }

    fn visit_local_var(&mut self, node: &LocalVarDecl) -> StructuralTraversal {
        for declarator in &node.declarators {
            self.scopes.declare(&declarator.name.name);
        }
        StructuralTraversal::Continue
    }

    fn visit_local_func_decl(&mut self, node: &LocalFuncDecl) -> StructuralTraversal {
        self.scopes.declare(&node.name.name);
        self.scopes.push();
        self.scopes.params(&node.params);
        StructuralTraversal::Continue
    }

    fn exit_local_func_decl(&mut self, _node: &LocalFuncDecl) {
        self.scopes.pop();
    }

    fn visit_for_init(&mut self, node: &ForInit) -> StructuralTraversal {
        if self.statement_for_init(node)
            && let ForInit::ForIn { declares_variable: true, name, .. } = node
        {
            self.scopes.declare(&name.name);
        }
        StructuralTraversal::Continue
    }

    fn exit_pattern(&mut self, node: &Pattern) {
        if self.binding_patterns.last().copied() == Some(std::ptr::from_ref(node)) {
            self.binding_patterns.pop();
            self.scopes.pattern(node);
        }
    }

    fn visit_pattern_declaration(&mut self, node: &PatternDeclaration) -> StructuralTraversal {
        self.scopes.pattern(&node.pattern);
        let start = self.diagnostics.len();
        self.pattern_declarations.push((start, start));
        StructuralTraversal::Continue
    }

    fn exit_pattern_declaration(&mut self, _node: &PatternDeclaration) {
        let (start, split) = self.pattern_declarations.pop().expect("pattern declaration");
        // Report initializer diagnostics before pattern diagnostics, even though
        // the structural walk visits the pattern first.
        self.diagnostics[start..].rotate_left(split - start);
    }

    fn visit_switch_case(&mut self, node: &SwitchCase) -> StructuralTraversal {
        self.scopes.push();
        self.prebind(&node.body);
        StructuralTraversal::Continue
    }

    fn exit_switch_case(&mut self, _node: &SwitchCase) {
        self.scopes.pop();
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

    fn visit_expr(&mut self, node: &Expr) -> StructuralTraversal {
        if let Expr::Field { object, field, .. } = node
            && matches!(object.unparenthesized(), Expr::This { .. })
            && !self.scopes.bound(&field.name)
            && (self.names.contains(&field.name)
                || !library_contains_name(self.program, self.model.as_ref(), &field.name))
        {
            self.diagnostics.push(self.ctx.diagnostic(
                "unnecessary-this",
                "Unnecessary 'this.' qualifier.",
                *object.span(),
            ));
        }
        if let Expr::FuncExpr { params, .. } = node {
            self.scopes.push();
            self.scopes.params(params);
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
