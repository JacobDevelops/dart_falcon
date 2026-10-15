//! Flags parentheses that can be removed without changing the meaning or
//! hurting readability.
//!
//! Parentheses are always reported when they sit directly inside other
//! parentheses, a string interpolation, the only argument of an argument list,
//! or the condition of an `if`, `while`, `do`, or `switch`. Elsewhere they are
//! reported around a single token (identifier, literal) or a null-aware chain
//! unless they are needed: before `.hashCode`/`.runtimeType`/`.toString()`/
//! `.noSuchMethod()`, before `!`, before a member of a nullable-type extension,
//! or around a null-aware index in a conditional `then` branch or map key.
//!
//! Parentheses are kept around conditional expressions, type literals,
//! cascades and anything inside a cascade, function expressions in a constructor
//! field initializer, `==`/`!=` comparisons that are assigned, returned, or
//! yielded, a `switch` expression starting a statement, and expressions with
//! internal whitespace (binary, `as`, `is`, `await`, assignments, ...) unless
//! the parentheses are the whole right-hand side, return value, argument, or
//! record field. Inside another expression they are kept when the parent is a
//! binary, conditional, `as`, `is`, or function invocation, before a member
//! access on a prefix/postfix expression, and where removing them would turn a
//! set or map literal into a statement block.
//!
//! Generic constructor tear-offs need parentheses when type arguments are
//! followed by invocation arguments (`(C.named)<T>()`), but not when the
//! instantiated tear-off is used as a value (`(C.named)<T>`). When constructor
//! signatures are unavailable, a type-qualified target is kept unless it is
//! known to be a nonconstructor member.
//!
//! Explicit expression grouping is retained by the parser, including parentheses
//! separated from their expression by comments. Pattern grouping is not an
//! expression and is not reported. The official exemption for a parenthesized
//! non-record argument to a record-typed parameter (a missing trailing comma)
//! only applies to code that does not compile and is not modeled here.
//!
//! ```dart
//! a = (b);          // flagged
//! print((x));       // flagged
//! a = (b == c);     // ok
//! (b ? c : d).foo(); // ok
//! ```
//!
//! Adopted from the official `unnecessary_parenthesis` lint.

use std::collections::HashSet;

use falcon_analyze::{AnalyzeContext, Rule, SemanticModel, resolve_call_constructor};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{
    Visitor, for_each_expr, walk_class_member, walk_constructor_decl, walk_expr, walk_field_decl,
    walk_function_decl, walk_getter_decl, walk_method_decl, walk_setter_decl, walk_stmt,
    walk_switch_case, walk_top_level_decl,
};

use crate::lint::expressions::is_parenthesized;

#[derive(Default)]
pub struct UnnecessaryParenthesis;

impl Rule for UnnecessaryParenthesis {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut collector = Collector {
            ctx,
            model: ctx
                .identities
                .map(|identities| SemanticModel::new(ctx.file_path, identities, ctx.types)),
            nullable_extension_members: nullable_extension_members(program),
            field_init_values: HashSet::new(),
            in_cascade: vec![false],
            bare: vec![false],
            statement_start: vec![None],
            under_prefix: HashSet::new(),
            method_callees: HashSet::new(),
            diagnostics: Vec::new(),
        };
        collector.visit_program(program);
        collector.diagnostics
    }
}

/// Members of extensions in this file whose extended type is nullable.
fn nullable_extension_members(program: &Program) -> HashSet<String> {
    let mut names = HashSet::new();
    for declaration in &program.declarations {
        let TopLevelDecl::Extension(extension) = declaration else {
            continue;
        };
        if !(extension.on_type.is_nullable()
            || matches!(extension.on_type, DartType::Dynamic { .. }))
        {
            continue;
        }
        for member in &extension.members {
            let name = match member {
                ClassMember::Method(method) => &method.name,
                ClassMember::Getter(getter) => &getter.name,
                ClassMember::Field(_)
                | ClassMember::Constructor(_)
                | ClassMember::PrimaryConstructorBody(_)
                | ClassMember::Setter(_)
                | ClassMember::Operator(_)
                | ClassMember::Error(_) => continue,
            };
            names.insert(name.name.clone());
        }
    }
    names
}

/// The syntactic position of a parenthesized expression.
#[derive(Clone, Copy)]
enum Parent<'a> {
    /// A string interpolation `${...}`.
    Interpolation,
    /// The only argument of an argument list.
    SoleArgument,
    /// One of several positional arguments.
    Argument,
    /// The value of a named argument.
    NamedArgument,
    /// The condition or subject of `if`, `while`, `do`, `switch`, or an `if`
    /// element.
    Condition,
    VariableInitializer {
        record_typed: bool,
    },
    AssignedValue,
    Return,
    Yield,
    FieldInitializer,
    ExpressionBody,
    RecordField,
    ExpressionStatement,
    Binary,
    Conditional {
        then_branch: bool,
    },
    CascadeTarget,
    /// The function of a function-expression invocation `(f)(x)`.
    InvokedFunction {
        has_type_arguments: bool,
    },
    As,
    Is,
    /// The target of a property access or method invocation.
    MemberTarget {
        name: &'a str,
        is_method: bool,
        under_prefix: bool,
        /// Whether this access is itself null-aware.
        null_aware: bool,
    },
    Prefix,
    NullAssert,
    MapKey,
    /// Any other expression parent.
    OtherExpression,
    /// Any other non-expression parent.
    OtherNode,
}

impl Parent<'_> {
    fn is_expression(self) -> bool {
        matches!(
            self,
            Parent::NamedArgument
                | Parent::AssignedValue
                | Parent::RecordField
                | Parent::Binary
                | Parent::Conditional { .. }
                | Parent::CascadeTarget
                | Parent::InvokedFunction { .. }
                | Parent::As
                | Parent::Is
                | Parent::MemberTarget { .. }
                | Parent::Prefix
                | Parent::NullAssert
                | Parent::OtherExpression
        )
    }

    fn is_argument(self) -> bool {
        matches!(
            self,
            Parent::SoleArgument | Parent::Argument | Parent::NamedArgument
        )
    }
}

/// What a parenthesized expression wraps: a real expression, or (for nested
/// parentheses) another parenthesized expression.
#[derive(Clone, Copy)]
enum Inner<'a> {
    Expr(&'a Expr),
    Parenthesized,
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    model: Option<SemanticModel<'a>>,
    nullable_extension_members: HashSet<String>,
    /// Spans of constructor field initializer values.
    field_init_values: HashSet<(usize, usize)>,
    /// Whether the nearest statement-or-cascade ancestor is a cascade.
    in_cascade: Vec<bool>,
    /// Whether the current node is "bare" inside a constructor field
    /// initializer (no function body or method invocation in between).
    bare: Vec<bool>,
    /// Start offset of the enclosing expression statement, if the nearest
    /// statement is one.
    statement_start: Vec<Option<usize>>,
    /// Spans of member accesses whose parent is a prefix expression.
    under_prefix: HashSet<(usize, usize)>,
    /// Spans of property accesses that are the callee of a method invocation,
    /// whose target is checked as a method-invocation target instead.
    method_callees: HashSet<(usize, usize)>,
    diagnostics: Vec<Diagnostic>,
}

fn key(span: &Span) -> (usize, usize) {
    (span.start, span.end)
}

impl Collector<'_> {
    fn report(&mut self, span: Span) {
        self.diagnostics.push(self.ctx.diagnostic(
            "unnecessary-parenthesis",
            "Unnecessary use of parentheses.",
            span,
        ));
    }

    /// Check explicit grouping; surrounding grammar delimiters are not nodes.
    fn check(&mut self, child: &Expr, parent: Parent<'_>) {
        let Expr::Paren { expr, span } = child else {
            return;
        };
        let outer = *span;
        let mut child = expr.as_ref();
        let mut nested = false;
        while let Expr::Paren { expr, span } = child {
            self.report(*span);
            child = expr;
            nested = true;
        }
        let inner = if nested {
            Inner::Parenthesized
        } else {
            Inner::Expr(child)
        };
        if self.is_unnecessary(outer, inner, parent) {
            self.report(outer);
        }
    }

    fn is_unnecessary(&self, node: Span, inner: Inner<'_>, parent: Parent<'_>) -> bool {
        let expression = match inner {
            Inner::Expr(expression) => Some(expression),
            Inner::Parenthesized => None,
        };
        // `(int,) r = (3);`: a record missing its trailing comma.
        if let Parent::VariableInitializer { record_typed: true } = parent
            && !matches!(expression, Some(Expr::Record { .. }))
        {
            return false;
        }
        if matches!(
            parent,
            Parent::Interpolation | Parent::SoleArgument | Parent::Condition
        ) {
            return true;
        }
        if let Some(expression) = expression {
            if matches!(expression, Expr::Conditional { .. })
                || is_type_literal(expression, self.model.as_ref())
            {
                return false;
            }
            // `C.named<T>()` is not constructor-invocation syntax: the
            // parentheses make this an invocation of a generic tear-off.
            if matches!(
                parent,
                Parent::InvokedFunction {
                    has_type_arguments: true
                }
            ) && let Some(model) = self.model.as_ref()
                && let Some((identity, name)) = resolve_call_constructor(expression, model, true)
                && self.ctx.signatures.is_none_or(|signatures| {
                    let identity = model.unaliased(identity);
                    signatures.member(&identity, name).is_none()
                        && signatures.declared_member_facts(&identity, name).is_empty()
                })
            {
                return false;
            }
            let null_aware = contains_null_aware_invocation(expression);
            if is_one_token(expression) || null_aware {
                return self.one_token_or_null_aware(expression, null_aware, parent);
            }
            // Bare or instantiated constructor tear-offs.
            if let Expr::Field { object, field, .. } = expression
                && (field.name == "new"
                    || matches!(object.as_ref(), Expr::GenericInstantiation { target, .. }
                        if matches!(target.as_ref(), Expr::Ident(_))))
                && !matches!(
                    parent,
                    Parent::InvokedFunction {
                        has_type_arguments: true
                    }
                )
            {
                return true;
            }
        }
        // `a..b = (c..d)` and anything inside a cascade.
        if matches!(expression, Some(Expr::Cascade { .. }))
            || self.in_cascade.last().copied().unwrap_or(false)
        {
            return false;
        }
        if self.bare_in_field_initializer(parent) && contains_function_expression(expression) {
            return false;
        }
        if let Some(Expr::Binary {
            op: BinaryOp::EqEq | BinaryOp::NotEq,
            ..
        }) = expression
            && matches!(
                parent,
                Parent::AssignedValue
                    | Parent::VariableInitializer { .. }
                    | Parent::Return
                    | Parent::Yield
                    | Parent::FieldInitializer
            )
        {
            return false;
        }
        if matches!(parent, Parent::ExpressionStatement)
            && matches!(expression, Some(Expr::Switch { .. }))
        {
            return false;
        }
        if expression
            .is_some_and(|expression| directly_contains_whitespace(self.ctx.source, expression))
            && !matches!(
                parent,
                Parent::AssignedValue
                    | Parent::FieldInitializer
                    | Parent::ExpressionBody
                    | Parent::RecordField
                    | Parent::Return
                    | Parent::VariableInitializer { .. }
                    | Parent::Yield
            )
            && !parent.is_argument()
        {
            return false;
        }
        if parent.is_expression() {
            match parent {
                Parent::Binary
                | Parent::Conditional { .. }
                | Parent::CascadeTarget
                | Parent::As
                | Parent::Is => return false,
                Parent::InvokedFunction { .. } if !is_prefixed_identifier(expression) => {
                    return false;
                }
                Parent::MemberTarget { under_prefix, .. } => {
                    if under_prefix
                        && expression.is_some_and(|expression| {
                            directly_contains_whitespace(self.ctx.source, expression)
                        })
                    {
                        return false;
                    }
                    if matches!(
                        expression,
                        Some(
                            Expr::PostfixIncDec { .. }
                                | Expr::NullAssert { .. }
                                | Expr::Unary { .. }
                        )
                    ) {
                        return false;
                    }
                }
                _ => {}
            }
            // `({1, 2, 3}).forEach(print);` would parse as a block.
            if matches!(expression, Some(Expr::Set { .. } | Expr::Map { .. }))
                && self.statement_start.last().copied().flatten() == Some(node.start)
            {
                return false;
            }
        }
        true
    }

    fn one_token_or_null_aware(
        &self,
        expression: &Expr,
        null_aware: bool,
        parent: Parent<'_>,
    ) -> bool {
        match parent {
            Parent::MemberTarget {
                name,
                is_method,
                null_aware: access_null_aware,
                ..
            } => {
                let object_member = if is_method {
                    matches!(name, "noSuchMethod" | "toString")
                } else {
                    matches!(name, "hashCode" | "runtimeType")
                };
                // The parentheses stop null-aware shorting, which is only valid
                // when the member exists on a nullable receiver, i.e. an
                // extension on a nullable type (possibly imported). A null-aware
                // access does not require the receiver's member to accept null.
                !object_member
                    && !(null_aware && !access_null_aware)
                    && !self.nullable_extension_members.contains(name)
            }
            Parent::NullAssert => false,
            Parent::Conditional { then_branch: true } | Parent::MapKey
                if matches!(
                    expression,
                    Expr::Index {
                        is_null_safe: true,
                        ..
                    }
                ) =>
            {
                false
            }
            _ => true,
        }
    }

    fn bare_in_field_initializer(&self, parent: Parent<'_>) -> bool {
        matches!(parent, Parent::FieldInitializer) || self.bare.last().copied().unwrap_or(false)
    }

    fn check_args(&mut self, args: &ArgList) {
        let sole = args.positional.len() + args.named.len() == 1;
        for argument in &args.positional {
            if sole {
                self.check(argument, Parent::SoleArgument);
            } else {
                self.check(argument, Parent::Argument);
            }
        }
        for argument in &args.named {
            self.check(&argument.value, Parent::NamedArgument);
        }
    }

    fn check_body(&mut self, body: Option<&FunctionBody>) {
        if let Some(FunctionBody::Arrow(expression, _)) = body {
            self.check(expression, Parent::ExpressionBody);
        }
    }

    fn check_condition(&mut self, condition: &IfCondition) {
        match condition {
            IfCondition::Expr(expression) => self.check(expression, Parent::Condition),
            IfCondition::Case(subject, _, guard) => {
                self.check(subject, Parent::Condition);
                if let Some(guard) = guard {
                    self.check(guard, Parent::OtherNode);
                }
            }
        }
    }

    fn check_declarators(&mut self, declared: Option<&DartType>, declarators: &[VarDeclarator]) {
        let record_typed = matches!(declared, Some(DartType::Record(_)));
        for declarator in declarators {
            if let Some(initializer) = &declarator.initializer {
                self.check(initializer, Parent::VariableInitializer { record_typed });
            }
        }
    }

    /// Checks the elements of a list or set literal, including those nested in
    /// `if` and `for` elements.
    fn check_elements(&mut self, elements: &[CollectionElement]) {
        let mut pending: Vec<&CollectionElement> = elements.iter().rev().collect();
        while let Some(element) = pending.pop() {
            match element {
                CollectionElement::Expr(expression) => {
                    self.check(expression, Parent::OtherExpression);
                }
                CollectionElement::NullAware { .. } | CollectionElement::Spread { .. } => {}
                CollectionElement::If {
                    condition,
                    then_elem,
                    else_elem,
                    ..
                } => {
                    self.check_condition(condition);
                    if let Some(else_elem) = else_elem {
                        pending.push(else_elem);
                    }
                    pending.push(then_elem);
                }
                CollectionElement::For { element, .. } => pending.push(element),
            }
        }
    }

    /// Checks the entries of a map literal, including those nested in `if` and
    /// `for` elements.
    fn check_entries(&mut self, elements: &[MapElement]) {
        let mut pending: Vec<&MapElement> = elements.iter().rev().collect();
        while let Some(element) = pending.pop() {
            match element {
                MapElement::Entry(entry) => {
                    if !entry.key_null_aware {
                        self.check(&entry.key, Parent::MapKey);
                    }
                    if !entry.value_null_aware {
                        self.check(&entry.value, Parent::OtherNode);
                    }
                }
                MapElement::Spread { .. } => {}
                MapElement::If {
                    condition,
                    then_entry,
                    else_entry,
                    ..
                } => {
                    self.check_condition(condition);
                    if let Some(else_entry) = else_entry {
                        pending.push(else_entry);
                    }
                    pending.push(then_entry);
                }
                MapElement::For { entry, .. } => pending.push(entry),
            }
        }
    }

    /// Checks the direct sub-expressions of `node`.
    fn check_children(&mut self, node: &Expr) {
        match node {
            Expr::Unary { operand, .. } => {
                if let Expr::Field { span, .. } | Expr::Call { span, .. } = operand.as_ref() {
                    self.under_prefix.insert(key(span));
                }
                self.check(operand, Parent::Prefix);
            }
            Expr::PostfixIncDec { operand, .. } => {
                self.check(operand, Parent::OtherExpression);
            }
            Expr::NullAssert { operand, .. } => self.check(operand, Parent::NullAssert),
            Expr::Binary { left, right, .. } => {
                self.check(left, Parent::Binary);
                self.check(right, Parent::Binary);
            }
            Expr::Assign { value, .. } => self.check(value, Parent::AssignedValue),
            Expr::Conditional {
                condition,
                then_expr,
                else_expr,
                ..
            } => {
                self.check(condition, Parent::Conditional { then_branch: false });
                self.check(then_expr, Parent::Conditional { then_branch: true });
                self.check(else_expr, Parent::Conditional { then_branch: false });
            }
            Expr::Is { expr, .. } => self.check(expr, Parent::Is),
            Expr::As { expr, .. } => self.check(expr, Parent::As),
            Expr::Field {
                object,
                field,
                is_null_safe,
                span,
            } if !self.method_callees.contains(&key(span)) => {
                let parent = Parent::MemberTarget {
                    name: &field.name,
                    is_method: false,
                    under_prefix: self.under_prefix.contains(&key(span)),
                    null_aware: *is_null_safe,
                };
                self.check(object, parent);
            }
            Expr::Field { .. } => {}
            Expr::Index { object, index, .. } => {
                self.check(object, Parent::OtherExpression);
                self.check(index, Parent::OtherExpression);
            }
            Expr::Call {
                callee,
                type_args,
                args,
                span,
            } => {
                // A wrapped callee is a function-expression invocation.
                let parenthesized = is_parenthesized(callee);
                match callee.as_ref() {
                    Expr::Field {
                        object,
                        field,
                        is_null_safe,
                        span: callee_span,
                    } if !parenthesized => {
                        self.method_callees.insert(key(callee_span));
                        let parent = Parent::MemberTarget {
                            name: &field.name,
                            is_method: true,
                            under_prefix: self.under_prefix.contains(&key(span)),
                            null_aware: *is_null_safe,
                        };
                        self.check(object, parent);
                    }
                    Expr::Ident(_) if !parenthesized => {}
                    callee => self.check(
                        callee,
                        Parent::InvokedFunction {
                            has_type_arguments: !type_args.is_empty(),
                        },
                    ),
                }
                self.check_args(args);
            }
            Expr::New { args, .. } => self.check_args(args),
            Expr::Cascade {
                object, sections, ..
            } => {
                self.check(object, Parent::CascadeTarget);
                for section in sections {
                    for op in &section.ops {
                        match op {
                            CascadeOp::Field { .. } | CascadeOp::NullAssert { .. } => {}
                            CascadeOp::Index { index, .. } => {
                                self.check(index, Parent::OtherExpression);
                            }
                            CascadeOp::Call(_, _, args) | CascadeOp::Invoke(_, args) => {
                                self.check_args(args);
                            }
                            CascadeOp::Assign(_, _, value)
                            | CascadeOp::IndexAssign(_, _, value) => {
                                self.check(value, Parent::AssignedValue);
                            }
                        }
                    }
                }
            }
            Expr::List { elements, .. } | Expr::Set { elements, .. } => {
                self.check_elements(elements);
            }
            Expr::Map { elements, .. } => self.check_entries(elements),
            Expr::Record { fields, .. } => {
                for field in fields.iter().filter(|field| field.name.is_none()) {
                    self.check(&field.value, Parent::RecordField);
                }
            }
            Expr::FuncExpr { body, .. } => self.check_body(Some(body)),
            Expr::Await { expr, .. } | Expr::Throw { expr, .. } => {
                self.check(expr, Parent::OtherExpression);
            }
            Expr::GenericInstantiation { target, .. } => {
                self.check(target, Parent::OtherExpression);
            }
            Expr::Switch { subject, arms, .. } => {
                self.check(subject, Parent::Condition);
                for arm in arms {
                    if let Some(guard) = &arm.guard {
                        self.check(guard, Parent::OtherNode);
                    }
                    self.check(&arm.body, Parent::OtherNode);
                }
            }
            Expr::StringLit(string) => {
                for interpolation in &string.interpolations {
                    self.check(&interpolation.expr, Parent::Interpolation);
                }
            }
            Expr::Paren { .. } => {}
            Expr::IntLit { .. }
            | Expr::DoubleLit { .. }
            | Expr::BoolLit { .. }
            | Expr::NullLit { .. }
            | Expr::Ident(_)
            | Expr::This { .. }
            | Expr::Super { .. }
            | Expr::DotShorthand { .. }
            | Expr::SymbolLit { .. }
            | Expr::Error { .. } => {}
        }
    }
}

impl Visitor for Collector<'_> {
    fn visit_switch_case(&mut self, node: &SwitchCase) {
        for case in &node.cases {
            if let SwitchCaseKind::Expr(expression) = case {
                self.check(expression, Parent::OtherNode);
            }
        }
        walk_switch_case(self, node);
    }

    fn visit_top_level_decl(&mut self, node: &TopLevelDecl) {
        if let TopLevelDecl::Variable(variable) = node {
            self.check_declarators(variable.var_type.as_ref(), &variable.declarators);
        }
        walk_top_level_decl(self, node);
    }

    fn visit_field_decl(&mut self, node: &FieldDecl) {
        self.check_declarators(node.field_type.as_ref(), &node.declarators);
        walk_field_decl(self, node);
    }

    fn visit_function_decl(&mut self, node: &FunctionDecl) {
        self.check_body(node.body.as_ref());
        walk_function_decl(self, node);
    }

    fn visit_method_decl(&mut self, node: &MethodDecl) {
        self.check_body(node.body.as_ref());
        walk_method_decl(self, node);
    }

    fn visit_getter_decl(&mut self, node: &GetterDecl) {
        self.check_body(node.body.as_ref());
        walk_getter_decl(self, node);
    }

    fn visit_setter_decl(&mut self, node: &SetterDecl) {
        self.check_body(node.body.as_ref());
        walk_setter_decl(self, node);
    }

    fn visit_class_member(&mut self, node: &ClassMember) {
        if let ClassMember::Operator(operator) = node {
            self.check_body(operator.body.as_ref());
        }
        walk_class_member(self, node);
    }

    fn visit_constructor_decl(&mut self, node: &ConstructorDecl) {
        for initializer in &node.initializers {
            match initializer {
                ConstructorInitializer::SuperCall { args, .. }
                | ConstructorInitializer::ThisCall { args, .. } => self.check_args(args),
                ConstructorInitializer::FieldInit { value, .. } => {
                    self.field_init_values.insert(key(value.span()));
                    self.check(value, Parent::FieldInitializer);
                }
                ConstructorInitializer::Assert {
                    condition, message, ..
                } => {
                    self.check(condition, Parent::OtherNode);
                    if let Some(message) = message {
                        self.check(message, Parent::OtherNode);
                    }
                }
            }
        }
        self.check_body(node.body.as_ref());
        walk_constructor_decl(self, node);
    }

    fn visit_stmt(&mut self, node: &Stmt) {
        // A statement is the nearest statement-or-cascade ancestor of everything
        // inside it, including its own direct expressions.
        self.in_cascade.push(false);
        self.bare.push(false);
        match node {
            Stmt::Expr(statement) => {
                self.statement_start.push(Some(statement.span.start));
                self.check(&statement.expr, Parent::ExpressionStatement);
            }
            _ => self.statement_start.push(None),
        }
        match node {
            Stmt::If(statement) => self.check_condition(&statement.condition),
            Stmt::While(statement) => self.check(&statement.condition, Parent::Condition),
            Stmt::DoWhile(statement) => self.check(&statement.condition, Parent::Condition),
            Stmt::Switch(statement) => self.check(&statement.subject, Parent::Condition),
            Stmt::Return(statement) => {
                if let Some(value) = &statement.value {
                    self.check(value, Parent::Return);
                }
            }
            Stmt::Yield(statement) => self.check(&statement.value, Parent::Yield),
            Stmt::Throw(statement) => self.check(&statement.value, Parent::OtherExpression),
            Stmt::LocalVar(declaration) => {
                self.check_declarators(declaration.var_type.as_ref(), &declaration.declarators);
            }
            Stmt::PatternDecl(declaration) => {
                // Pattern declarations are not ordinary variable declarations:
                // retain the non-expression-parent whitespace exemptions.
                self.check(&declaration.init, Parent::OtherNode);
            }
            Stmt::Assert(statement) => {
                self.check(&statement.condition, Parent::OtherNode);
                if let Some(message) = &statement.message {
                    self.check(message, Parent::OtherNode);
                }
            }
            Stmt::LocalFunc(function) => self.check_body(Some(&function.body)),
            _ => {}
        }
        walk_stmt(self, node);
        self.bare.pop();
        self.in_cascade.pop();
        self.statement_start.pop();
    }

    fn visit_expr(&mut self, node: &Expr) {
        let bare = if self.field_init_values.contains(&key(node.span())) {
            true
        } else if is_function_boundary(node) {
            false
        } else {
            self.bare.last().copied().unwrap_or(false)
        };
        let in_cascade = matches!(node, Expr::Cascade { .. })
            || self.in_cascade.last().copied().unwrap_or(false);
        self.bare.push(bare);
        self.in_cascade.push(in_cascade);
        self.check_children(node);
        walk_expr(self, node);
        self.in_cascade.pop();
        self.bare.pop();
    }
}

/// A function expression, or a method invocation (whose argument parentheses
/// delimit any `=>` inside them). A call of a capitalized name may be an
/// instance creation, which does not delimit.
fn is_function_boundary(node: &Expr) -> bool {
    match node {
        Expr::FuncExpr { .. } => true,
        Expr::Call { callee, .. } => match callee.as_ref() {
            Expr::Ident(identifier) => !starts_uppercase(&identifier.name),
            Expr::Field { .. } => true,
            _ => false,
        },
        _ => false,
    }
}

fn starts_uppercase(name: &str) -> bool {
    name.trim_start_matches(['_', '$'])
        .chars()
        .next()
        .is_some_and(char::is_uppercase)
}

/// A written type literal; resolve lowercase type names (`int`) as well as
/// generic types. Capitalization is only a fallback without resolved identity.
fn is_type_literal(expression: &Expr, model: Option<&SemanticModel<'_>>) -> bool {
    let is_type_name = |identifier: &Identifier| match model {
        Some(model) => model.resolve_name(identifier.name.as_str()).is_some(),
        None => starts_uppercase(&identifier.name),
    };
    match expression {
        Expr::Ident(identifier) => is_type_name(identifier),
        Expr::GenericInstantiation { target, .. } => {
            matches!(target.as_ref(), Expr::Ident(identifier) if is_type_name(identifier))
        }
        _ => false,
    }
}

fn is_one_token(expression: &Expr) -> bool {
    matches!(
        expression,
        Expr::Ident(_)
            | Expr::StringLit(_)
            | Expr::IntLit { .. }
            | Expr::DoubleLit { .. }
            | Expr::NullLit { .. }
            | Expr::BoolLit { .. }
    )
}

fn is_prefixed_identifier(expression: Option<&Expr>) -> bool {
    matches!(
        expression,
        Some(Expr::Field {
            object,
            is_null_safe: false,
            ..
        }) if matches!(object.as_ref(), Expr::Ident(_))
    )
}

/// Whether a property access, method invocation, or index chain contains a
/// null-aware link (`?.` or `?[`).
fn contains_null_aware_invocation(expression: &Expr) -> bool {
    let mut current = expression;
    loop {
        let (null_aware, target) = match current {
            Expr::Field {
                object,
                is_null_safe,
                ..
            }
            | Expr::Index {
                object,
                is_null_safe,
                ..
            } => (*is_null_safe, object),
            Expr::Call { callee, .. } => match callee.as_ref() {
                Expr::Field {
                    object,
                    is_null_safe,
                    ..
                } => (*is_null_safe, object),
                _ => return false,
            },
            _ => return false,
        };
        if null_aware {
            return true;
        }
        current = target;
    }
}

/// The analyzer's `directlyContainsWhitespace`.
fn directly_contains_whitespace(source: &str, expression: &Expr) -> bool {
    let mut current = expression;
    loop {
        match current {
            Expr::As { .. }
            | Expr::Assign { .. }
            | Expr::Await { .. }
            | Expr::Binary { .. }
            | Expr::FuncExpr { .. }
            | Expr::Is { .. }
            | Expr::Switch { .. } => return true,
            Expr::New { span, .. } => {
                let text = source.get(span.start..span.end).unwrap_or_default();
                return text.starts_with("new") || text.starts_with("const");
            }
            Expr::List { is_const, span, .. }
            | Expr::Map { is_const, span, .. }
            | Expr::Set { is_const, span, .. } => {
                return *is_const
                    && source
                        .get(span.start..span.end)
                        .is_some_and(|text| text.starts_with("const"));
            }
            Expr::Field { object, .. } => current = object,
            Expr::Call { callee, .. } => match callee.as_ref() {
                Expr::Field { object, .. } => current = object,
                _ => return false,
            },
            _ => return false,
        }
    }
}

fn contains_function_expression(expression: Option<&Expr>) -> bool {
    let Some(expression) = expression else {
        // Nested parentheses: the caller's child is checked separately.
        return false;
    };
    let mut found = false;
    for_each_expr(expression, &mut |node| {
        found |= matches!(node, Expr::FuncExpr { .. });
    });
    found
}
