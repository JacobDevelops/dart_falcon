use std::collections::HashSet;

use crate::lint::lexical_usage::NameScopes;
use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, QualifiedName, ResolvedType, ResolvedTypeView,
    RulePreparation, SemanticModel, SignatureIndex, StructuralNode, StructuralRuleVisitor,
    StructuralTraversal, Truth, TypeRelation,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::visit::{Visitor, walk_expr, walk_stmt};
use falcon_syntax::cst::*;
pub(crate) const TIMER_RELEASE: crate::lint::resource_leaks::Release =
    crate::lint::resource_leaks::Release {
        library: "dart:async",
        type_name: "Timer",
        method: "cancel",
    };
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum DisposalKind {
    Controller,
    FocusAndTimer,
    OtherFields,
}

pub(crate) fn prepare_disposal<'a>(
    ctx: &'a AnalyzeContext<'a>,
    kind: DisposalKind,
) -> RulePreparation<'a> {
    let (Some(identities), Some(signatures)) = (ctx.identities(), ctx.signatures()) else {
        return RulePreparation::Diagnostics(Vec::new());
    };
    RulePreparation::Structural(Box::new(Collector {
        ctx,
        model: SemanticModel::new(ctx.file_path(), ctx.parsed(), identities, ctx.types()),
        signatures,
        diagnostics: Vec::new(),
        fields: HashSet::new(),
        owned: HashSet::new(),
        disposed: HashSet::new(),
        cancelled: HashSet::new(),
        scopes: NameScopes::default(),
        in_dispose: false,
        active_owner: false,
        case_patterns: Vec::new(),
        if_conditions: Vec::new(),
        statement_header: false,
        headers: Vec::new(),
        kind,
        direct_releases: HashSet::new(),
        timers: HashSet::new(),
    }))
}

pub(crate) fn is_flutter_state(
    name: &str,
    signatures: &SignatureIndex,
    model: &SemanticModel<'_>,
) -> bool {
    let Some(identity) = model.resolve_name(name) else {
        return false;
    };
    let ty = ResolvedType::interface_identity(identity);
    let state = DeclarationIdentity::flutter("State");
    signatures.relation(&ty, TypeRelation::SubtypeOfDeclaration(&state), model) == Truth::Yes
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    model: SemanticModel<'a>,
    signatures: &'a SignatureIndex,
    diagnostics: Vec<Diagnostic>,
    fields: HashSet<String>,
    owned: HashSet<String>,
    disposed: HashSet<String>,
    cancelled: HashSet<String>,
    scopes: NameScopes,
    in_dispose: bool,
    active_owner: bool,
    case_patterns: Vec<Span>,
    if_conditions: Vec<NodeId>,
    statement_header: bool,
    headers: Vec<bool>,
    kind: DisposalKind,
    direct_releases: HashSet<Span>,
    timers: HashSet<String>,
}

impl Collector<'_> {
    fn constructs_resource(&self, expression: Expr<'_>) -> bool {
        if self.kind == DisposalKind::Controller {
            return constructs_controller(expression, &self.model);
        }
        let Some(identity) = constructed_identity(expression, &self.model, self.signatures) else {
            return false;
        };
        let ty = ResolvedType::interface_identity(identity);
        let special = self.signatures.relation(
            &ty,
            TypeRelation::SubtypeOfDeclaration(&DeclarationIdentity::flutter("FocusNode")),
            &self.model,
        ) == Truth::Yes
            || ty.interface(TIMER_RELEASE.library, TIMER_RELEASE.type_name);
        if self.kind == DisposalKind::FocusAndTimer {
            return special;
        }
        !special
            && !constructs_controller(expression, &self.model)
            && has_dispose(&ty, self.signatures, &self.model)
    }
    fn field_target<'b>(&self, expression: Expr<'b>) -> Option<&'b str> {
        let mut expression = expression.unparenthesized();
        while let Expr::NullAssert(assertion) = expression {
            expression = assertion.operand().unparenthesized();
        }
        match expression {
            Expr::Field(field)
                if matches!(field.object().unparenthesized(), Expr::This(_))
                    && self.fields.contains(field.field().name()) =>
            {
                Some(field.field().name())
            }
            Expr::Ident(identifier)
                if self.fields.contains(identifier.name())
                    && !self.scopes.bound(identifier.name()) =>
            {
                Some(identifier.name())
            }
            _ => None,
        }
    }
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }

    fn visit_top_level_decl(&mut self, node: TopLevelDecl<'a>) -> StructuralTraversal {
        self.active_owner = false;
        let (name, members) = match node {
            TopLevelDecl::Class(class) => (class.name().name(), class.members()),
            TopLevelDecl::MixinClass(class) => (class.name().name(), class.members()),
            _ => return StructuralTraversal::SkipChildren,
        };
        if !is_flutter_state(name, self.signatures, &self.model) {
            return StructuralTraversal::SkipChildren;
        }
        self.fields.clear();
        self.owned.clear();
        self.active_owner = true;
        self.timers.clear();
        self.disposed.clear();
        self.cancelled.clear();
        // Member names and initial ownership must be available before any body.
        for member in members {
            if let ClassMember::Field(field) = member {
                for declarator in field.declarators() {
                    if !field.is_static() {
                        self.fields.insert(declarator.name().name().to_owned());
                    }
                    if (self.kind == DisposalKind::Controller || !field.is_static()) && declarator.initializer().is_some_and(|initializer| {
                        self.constructs_resource(initializer)
                    }) {
                        self.owned.insert(declarator.name().name().to_owned());
                        if declarator.initializer().is_some_and(|value| {
                            constructed_identity(value, &self.model, self.signatures).is_some_and(|identity| ResolvedType::interface_identity(identity).interface("dart:async", "Timer"))
                        }) {
                            self.timers.insert(declarator.name().name().to_owned());
                        }
                    }
                }
            }
        }
        StructuralTraversal::Continue
    }

    fn exit_top_level_decl(&mut self, node: TopLevelDecl<'a>) {
        let members = match node {
            TopLevelDecl::Class(class) => class.members(),
            TopLevelDecl::MixinClass(class) => class.members(),
            _ => return,
        };
        if !self.active_owner {
            return;
        }
        for member in members {
            if let ClassMember::Field(field) = member {
                for declarator in field.declarators() {
                    if self.owned.contains(declarator.name().name())
                        && !(if self.timers.contains(declarator.name().name()) { &self.cancelled } else { &self.disposed }).contains(declarator.name().name())
                    {
                        let message = if self.kind == DisposalKind::Controller {
                            "This controller is never disposed; call its `dispose()` method in `dispose()`."
                        } else {
                            "This owned field is never released; call its `dispose()` or `cancel()` method in `dispose()`."
                        };
                        self.diagnostics.push(self.ctx.diagnostic(message, declarator.name().span()));
                    }
                }
            }
        }
    }

    fn visit_class_member(&mut self, node: ClassMember<'a>) -> StructuralTraversal {
        let body = match node {
            ClassMember::Method(method) => method.body(),
            ClassMember::Constructor(constructor) => constructor.body(),
            ClassMember::Getter(getter) => getter.body(),
            ClassMember::Setter(setter) => setter.body(),
            ClassMember::Operator(operator) => operator.body(),
            _ => None,
        };
        if body.is_none() {
            return StructuralTraversal::SkipChildren;
        }
        self.scopes.push();
        self.in_dispose = false;
        let params = match node {
            ClassMember::Method(method) => {
                self.in_dispose = method.name().name() == "dispose";
                self.direct_releases.clear();
                if self.in_dispose && let Some(body) = method.body() {
                    match body {
                        FunctionBody::Arrow(arrow) => {
                            EagerReleases(&mut self.direct_releases).visit_expr(arrow.expr());
                        }
                        FunctionBody::Block(block) => {
                            for statement in block.stmts() {
                                if let Stmt::Expr(expression) = statement {
                                    EagerReleases(&mut self.direct_releases).visit_expr(expression.expr());
                                }
                                let mut exits = EarlyExit(false);
                                exits.visit_stmt(statement);
                                if exits.0 { break; }
                            }
                        }
                        _ => {}
                    }
                }
                Some(method.params())
            }
            ClassMember::Constructor(constructor) => Some(constructor.params()),
            ClassMember::Operator(operator) => Some(operator.params()),
            ClassMember::Setter(setter) => {
                self.scopes.declare(setter.param().name());
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

    fn exit_class_member(&mut self, node: ClassMember<'a>) {
        let has_body = match node {
            ClassMember::Method(method) => method.body().is_some(),
            ClassMember::Constructor(constructor) => constructor.body().is_some(),
            ClassMember::Getter(getter) => getter.body().is_some(),
            ClassMember::Setter(setter) => setter.body().is_some(),
            ClassMember::Operator(operator) => operator.body().is_some(),
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
            | StructuralNode::Expr(Expr::FuncExpr(_)) => {
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
                if self.if_conditions.last().copied() == Some(condition.syntax().id()) =>
            {
                if let StructuralNode::Pattern(pattern) = child {
                    self.scopes.push();
                    self.case_patterns.push(pattern.span());
                }
                true
            }
            StructuralNode::IfStmt(statement) => {
                if let StructuralNode::Stmt(branch) = child
                    && matches!(statement.condition(), IfCondition::Case(..))
                    && statement.else_branch().is_some_and(|otherwise| {
                        otherwise.syntax().id() == branch.syntax().id()
                    })
                {
                    self.scopes.pop();
                }
                true
            }
            _ => true,
        }
    }

    fn visit_stmt(&mut self, node: Stmt<'a>) -> StructuralTraversal {
        if let Stmt::If(statement) = node {
            self.if_conditions.push(statement.condition().syntax().id());
        }
        match node {
            Stmt::Block(_) | Stmt::For(_) => self.scopes.push(),
            Stmt::LocalFunc(function) => {
                self.scopes.declare(function.name().name());
                self.scopes.push();
                self.scopes.params(function.params());
            }
            _ => {}
        }
        StructuralTraversal::Continue
    }

    fn exit_stmt(&mut self, node: Stmt<'a>) {
        if matches!(node, Stmt::If(_)) {
            self.if_conditions.pop();
        }
        if matches!(node, Stmt::Block(_) | Stmt::For(_) | Stmt::LocalFunc(_)) {
            self.scopes.pop();
        }
    }

    fn exit_if_stmt(&mut self, node: IfStmt<'a>) {
        if matches!(node.condition(), IfCondition::Case(..)) && node.else_branch().is_none() {
            self.scopes.pop();
        }
    }

    fn visit_catch_clause(&mut self, node: CatchClause<'a>) -> StructuralTraversal {
        self.scopes.push();
        if let Some(name) = node.exception_var() {
            self.scopes.declare(name.name());
        }
        if let Some(name) = node.stack_trace_var() {
            self.scopes.declare(name.name());
        }
        StructuralTraversal::Continue
    }

    fn exit_catch_clause(&mut self, _node: CatchClause<'a>) {
        self.scopes.pop();
    }

    fn exit_var_declarator(&mut self, node: VarDeclarator<'a>) {
        self.scopes.declare(node.name().name());
    }

    fn exit_pattern(&mut self, node: Pattern<'a>) {
        if self.case_patterns.last() == Some(&node.span()) {
            self.case_patterns.pop();
            self.scopes.pattern(node);
        }
    }

    fn exit_pattern_declaration(&mut self, node: PatternDeclaration<'a>) {
        self.scopes.pattern(node.pattern());
    }

    fn visit_for_header(&mut self, _node: ForHeader<'a>) -> StructuralTraversal {
        self.headers.push(std::mem::take(&mut self.statement_header));
        StructuralTraversal::Continue
    }

    fn exit_for_header(&mut self, _node: ForHeader<'a>) {
        self.headers.pop();
    }

    fn exit_for_init(&mut self, node: ForInit<'a>) {
        if self.headers.last() != Some(&true) {
            return;
        }
        match node {
            ForInit::ForIn(initializer) if initializer.declares_variable() => {
                self.scopes.declare(initializer.name().name());
            }
            ForInit::PatternForIn(initializer) => self.scopes.pattern(initializer.pattern()),
            _ => {}
        }
    }

    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        match node {
            Expr::Assign(assignment)
                if self.constructs_resource(assignment.value()) =>
            {
                if let Some(field) = self.field_target(assignment.target()) {
                    self.owned.insert(field.to_string());
                    if constructed_identity(assignment.value(), &self.model, self.signatures).is_some_and(|identity| ResolvedType::interface_identity(identity).interface("dart:async", "Timer")) {
                        self.timers.insert(field.to_string());
                    }
                }
            }
            Expr::Call(call) if self.in_dispose && (self.kind == DisposalKind::Controller || self.direct_releases.contains(&node.span())) => {
                if let Expr::Field(field) = call.callee().unparenthesized()
                    && matches!(field.field().name(), "dispose" | "cancel")
                    && let Some(controller) = self.field_target(field.object())
                {
                    if field.field().name() == "cancel" {
                        self.cancelled.insert(controller.to_string());
                    } else {
                        self.disposed.insert(controller.to_string());
                    }
                }
            }
            Expr::Cascade(cascade)
                if self.in_dispose && (self.kind == DisposalKind::Controller || self.direct_releases.contains(&node.span())) =>
            {
                if let Some(controller) = self.field_target(cascade.object()) {
                    let disposed = cascade.sections().any(|section| matches!(section.ops().first(), Some(CascadeOp::Call(call)) if call.name().name() == "dispose"));
                    let cancelled = cascade.sections().any(|section| matches!(section.ops().first(), Some(CascadeOp::Call(call)) if call.name().name() == "cancel"));
                    if disposed { self.disposed.insert(controller.to_string()); }
                    if cancelled { self.cancelled.insert(controller.to_string()); }
                }
            }
            Expr::FuncExpr(function) => {
                self.scopes.push();
                self.scopes.params(function.params());
            }
            _ => {}
        }
        StructuralTraversal::Continue
    }

    fn exit_expr(&mut self, node: Expr<'a>) {
        if matches!(node, Expr::FuncExpr(_)) {
            self.scopes.pop();
        }
    }
}
}

pub(crate) fn constructor_identity(
    callee: Expr<'_>,
    model: &SemanticModel<'_>,
) -> Option<DeclarationIdentity> {
    match callee.unparenthesized() {
        Expr::Ident(identifier) => model.resolve_name(identifier.name()),
        Expr::Field(field) => match field.object().unparenthesized() {
            Expr::Ident(identifier) => model
                .resolve_name(QualifiedName::prefixed(
                    identifier.name(),
                    field.field().name(),
                ))
                .or_else(|| model.resolve_name(identifier.name())),
            Expr::Field(type_name) => match type_name.object().unparenthesized() {
                Expr::Ident(prefix) => model.resolve_name(QualifiedName::prefixed(
                    prefix.name(),
                    type_name.field().name(),
                )),
                _ => None,
            },
            _ => None,
        },
        _ => None,
    }
}

pub(crate) fn constructed_identity(
    expression: Expr<'_>,
    model: &SemanticModel<'_>,
    signatures: &SignatureIndex,
) -> Option<DeclarationIdentity> {
    let mut expression = expression.unparenthesized();
    while let Expr::Cascade(cascade) = expression {
        expression = cascade.object().unparenthesized();
    }
    let (identity, name, _) = crate::lint::flutter::constructor_invocation(expression, model)?;
    if signatures.has_constructor(&identity, name)
        || (name == "new"
            && matches!(
                identity,
                DeclarationIdentity::Package { .. } | DeclarationIdentity::Sdk { .. }
            ))
        || (identity == DeclarationIdentity::sdk("dart:async", "Timer") && name == "periodic")
        || (identity == DeclarationIdentity::flutter("TextEditingController")
            && name == "fromValue")
    {
        Some(identity)
    } else {
        None
    }
}

pub(crate) fn has_dispose(
    ty: &ResolvedType,
    signatures: &SignatureIndex,
    model: &SemanticModel<'_>,
) -> bool {
    matches!(ty.view(), ResolvedTypeView::Interface { identity: DeclarationIdentity::Package { package, name, .. }, .. }
        if package.as_str() == "flutter" && matches!(name.as_str(), "FocusNode" | "ChangeNotifier" | "ValueNotifier" | "TextEditingController" | "ScrollController" | "PageController" | "TabController" | "AnimationController"))
        || signatures
            .resolved_member_facts(ty, "dispose", model)
            .is_some_and(|members| {
                members
                    .iter()
                    .any(|member| matches!(member.kind, falcon_analyze::MemberKind::Method))
            })
}

fn constructs_controller(expression: Expr<'_>, model: &SemanticModel<'_>) -> bool {
    let mut expression = expression.unparenthesized();
    while let Expr::Cascade(cascade) = expression {
        expression = cascade.object().unparenthesized();
    }
    let identity = match expression.unparenthesized() {
        Expr::New(construction) => match model.resolve_type(construction.dart_type()).view() {
            ResolvedTypeView::Interface { identity, .. } => Some(identity.clone()),
            _ => None,
        },
        Expr::Call(call) => constructor_identity(call.callee(), model),
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

/// A conditional early exit makes any later release non-guaranteed. Nested
/// functions execute separately and cannot exit the enclosing dispose method.
struct EarlyExit(bool);
impl<'a> Visitor<'a> for EarlyExit {
    fn visit_stmt(&mut self, statement: Stmt<'a>) {
        match statement {
            Stmt::Return(_) | Stmt::Throw(_) | Stmt::Rethrow(_) => self.0 = true,
            Stmt::LocalFunc(_) => {}
            _ if !self.0 => walk_stmt(self, statement),
            _ => {}
        }
    }
    fn visit_expr(&mut self, expression: Expr<'a>) {
        match expression {
            Expr::Throw(_) => self.0 = true,
            Expr::FuncExpr(_) => {}
            _ if !self.0 => walk_expr(self, expression),
            _ => {}
        }
    }
}

/// Record eagerly evaluated positions through the exhaustive CST walker. Each
/// scope can restrict traversal to one mandatory operand; deferred bodies and
/// conditional collection/cascade operands do not establish a release.
struct EagerReleases<'a>(&'a mut HashSet<Span>);
impl<'a> Visitor<'a> for EagerReleases<'_> {
    fn visit_expr(&mut self, expression: Expr<'a>) {
        EagerReleaseScope {
            releases: self.0,
            only: None,
            stopped: false,
            cascade_reached: 0,
            cascade_unreachable: 0,
        }
        .visit_expr(expression);
    }
}

struct EagerReleaseScope<'a> {
    releases: &'a mut HashSet<Span>,
    only: Option<Span>,
    stopped: bool,
    cascade_reached: u8,
    cascade_unreachable: u8,
}

impl EagerReleaseScope<'_> {
    fn note_exits(&mut self, expression: Expr<'_>) {
        let mut exits = EarlyExit(false);
        exits.visit_expr(expression);
        self.stopped |= exits.0;
    }
}

impl<'a> Visitor<'a> for EagerReleaseScope<'_> {
    fn visit_expr(&mut self, expression: Expr<'a>) {
        if self.stopped {
            return;
        }
        if self.only.is_some_and(|span| span != expression.span()) {
            self.note_exits(expression);
            return;
        }
        if matches!(expression, Expr::FuncExpr(_)) {
            return;
        }
        self.releases.insert(expression.span());
        let outer = self.only;
        self.only = match expression {
            Expr::Call(call) if null_short_circuits(call.callee()) => Some(call.callee().span()),
            Expr::Index(index) if index.is_null_safe() || null_short_circuits(index.object()) => {
                Some(index.object().span())
            }
            Expr::Assign(assignment)
                if assignment.op() == AssignOp::NullCoalesceEq
                    || null_short_circuits(assignment.target()) =>
            {
                Some(assignment.target().span())
            }
            Expr::Binary(binary)
                if matches!(
                    binary.op(),
                    BinaryOp::And | BinaryOp::Or | BinaryOp::NullCoalesce
                ) =>
            {
                Some(binary.left().span())
            }
            Expr::Conditional(conditional) => Some(conditional.condition().span()),
            Expr::Switch(switch) => Some(switch.subject().span()),
            _ => None,
        };
        let outer_cascade = (
            std::mem::take(&mut self.cascade_reached),
            std::mem::take(&mut self.cascade_unreachable),
        );
        walk_expr(self, expression);
        self.only = outer;
        self.stopped |= matches!(expression, Expr::Throw(_));
        let release_unreachable = self.cascade_unreachable & !self.cascade_reached != 0;
        (self.cascade_reached, self.cascade_unreachable) = outer_cascade;
        // Calls must reach their invocation; cascades may already have released
        // their receiver before a later section throws.
        if self.stopped && (!matches!(expression, Expr::Cascade(_)) || release_unreachable) {
            self.releases.remove(&expression.span());
        }
    }

    fn visit_element<E: Element<'a>>(&mut self, element: E) {
        if matches!(
            element.syntax().kind(),
            NodeKind::IfElement
                | NodeKind::ForElement
                | NodeKind::MapIfElement
                | NodeKind::MapForElement
        ) {
            let mut exits = EarlyExit(false);
            exits.visit_element(element);
            self.stopped |= exits.0;
        } else {
            falcon_syntax::cst::visit::walk_element(self, element.kind());
        }
    }

    fn visit_map_entry(&mut self, entry: MapEntry<'a>) {
        if entry.key_null_aware() {
            self.visit_expr(entry.key());
            self.note_exits(entry.value());
        } else {
            falcon_syntax::cst::visit::walk_map_entry(self, entry);
        }
    }

    fn visit_cascade_section(&mut self, section: CascadeSection<'a>) {
        // Keep direct receiver release detection, but do not infer eager calls
        // from section operands: null-aware chains and assignments can skip them.
        let mut exits = EarlyExit(self.stopped);
        for (index, op) in section.ops().enumerate() {
            exits.visit_cascade_op(op);
            if index == 0
                && let CascadeOp::Call(call) = op
            {
                let release = match call.name().name() {
                    "dispose" => 1,
                    "cancel" => 2,
                    _ => 0,
                };
                if exits.0 {
                    self.cascade_unreachable |= release;
                } else {
                    self.cascade_reached |= release;
                }
            }
        }
        self.stopped = exits.0;
    }
}

/// Null-aware accesses also short-circuit later arguments, indices and stores
/// in the same receiver chain, not only the immediately following operation.
fn null_short_circuits(mut expression: Expr<'_>) -> bool {
    loop {
        expression = match expression {
            Expr::Field(field) => {
                if field.is_null_safe() {
                    return true;
                }
                field.object()
            }
            Expr::Index(index) => {
                if index.is_null_safe() {
                    return true;
                }
                index.object()
            }
            Expr::Call(call) => call.callee(),
            Expr::GenericInstantiation(instantiation) => instantiation.target(),
            Expr::NullAssert(assertion) => assertion.operand(),
            _ => return false,
        };
    }
}
