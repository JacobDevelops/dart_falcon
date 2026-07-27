//! Requires an explicit `.call` tear-off when a callable object is used as a function.

use std::collections::{HashMap, HashSet};

use falcon_analyze::{
    AnalyzeContext, LocalTypes, MemberKind, MemberResult, ReceiverTypes, ResolvedType, Rule,
    SemanticMemberKind, StaticType, TypeIndex,
};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr, walk_stmt};

use crate::lint::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};

pub struct ImplicitCallTearoffs;

impl Rule for ImplicitCallTearoffs {
    fn name(&self) -> &'static str {
        "implicit-call-tearoffs"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let facts = ProgramFacts::from_program(program);
        let mut collector = Collector {
            diags: Vec::new(),
            file: ctx.file_path.to_string_lossy().into_owned(),
            types: ctx.types,
            facts,
            locals: LocalTypes::new(),
            local_values: LocalValues::new(),
            enclosing_type: None,
            current_static: false,
            returns_function: false,
        };
        collector.visit_program(program);
        let mut diags = collector.diags;
        if let Some(state) = SemanticState::new(program, ctx) {
            let mut supplemental = SemanticCollector {
                diags: Vec::new(),
                file: ctx.file_path.to_string_lossy().into_owned(),
            };
            visit_program(&mut supplemental, program, state);
            for diagnostic in supplemental.diags {
                if !diags.iter().any(|existing| {
                    existing.span.start == diagnostic.span.start
                        && existing.span.end == diagnostic.span.end
                }) {
                    diags.push(diagnostic);
                }
            }
        }
        diags
    }
}

#[derive(Clone, Default)]
struct Signature {
    positional: Vec<bool>,
    named: HashMap<String, bool>,
    returns_function: bool,
}

impl Signature {
    fn from_params(params: &FormalParamList, return_type: Option<&DartType>) -> Self {
        Self {
            positional: params
                .positional
                .iter()
                .chain(&params.optional_positional)
                .map(formal_expects_function)
                .collect(),
            named: params
                .named
                .iter()
                .map(|param| (param.name.name.clone(), formal_expects_function(param)))
                .collect(),
            returns_function: return_type.is_some_and(is_function_type),
        }
    }

    fn from_function_type(function: &FunctionType) -> Self {
        let mut positional = Vec::new();
        let mut named = HashMap::new();
        for param in &function.params {
            let expects_function = is_function_type(&param.param_type);
            if param.is_named {
                if let Some(name) = &param.name {
                    named.insert(name.name.clone(), expects_function);
                }
            } else {
                positional.push(expects_function);
            }
        }
        Self {
            positional,
            named,
            returns_function: function
                .return_type
                .as_deref()
                .is_some_and(is_function_type),
        }
    }

    fn from_formal(param: &FormalParam) -> Option<Self> {
        if let Some(params) = &param.function_params {
            return Some(Self::from_params(params, param.param_type.as_ref()));
        }
        signature_from_type(param.param_type.as_ref()?)
    }
}

#[derive(Clone)]
struct ValueFact {
    static_type: StaticType,
    expects_function: bool,
    signature: Option<Signature>,
    is_static: bool,
}

impl ValueFact {
    fn from_type(ty: Option<&DartType>, is_static: bool) -> Self {
        Self {
            static_type: ty.map(local_static_type).unwrap_or(StaticType::Unknown),
            expects_function: ty.is_some_and(is_function_type),
            signature: ty.and_then(signature_from_type),
            is_static,
        }
    }
}

#[derive(Clone)]
struct MethodFact {
    signature: Signature,
    is_static: bool,
}

#[derive(Clone)]
enum SuperclassFact {
    Local(String),
    Unknown,
}

#[derive(Clone, Default)]
struct TypeFacts {
    constructors: HashMap<Option<String>, Signature>,
    methods: HashMap<String, MethodFact>,
    fields: HashMap<String, ValueFact>,
    setters: HashMap<String, ValueFact>,
    superclass: Option<SuperclassFact>,
}

#[derive(Clone)]
enum Lookup<T> {
    Found(T),
    Absent,
    Unknown,
}

#[derive(Default)]
struct ProgramFacts {
    functions: HashMap<String, Signature>,
    variables: HashMap<String, ValueFact>,
    setters: HashMap<String, ValueFact>,
    types: HashMap<String, TypeFacts>,
}

impl ProgramFacts {
    fn from_program(program: &Program) -> Self {
        let mut facts = Self::default();
        for declaration in &program.declarations {
            match declaration {
                TopLevelDecl::Function(function) if function.is_setter => {
                    if let Some(param) = function.params.positional.first() {
                        facts.setters.insert(
                            function.name.name.clone(),
                            ValueFact::from_type(param.param_type.as_ref(), true),
                        );
                    }
                }
                TopLevelDecl::Function(function) if !function.is_getter => {
                    facts.functions.insert(
                        function.name.name.clone(),
                        Signature::from_params(&function.params, function.return_type.as_ref()),
                    );
                }
                TopLevelDecl::Variable(variable) => {
                    for declarator in &variable.declarators {
                        facts.variables.insert(
                            declarator.name.name.clone(),
                            ValueFact::from_type(variable.var_type.as_ref(), true),
                        );
                    }
                }
                TopLevelDecl::Class(class) => {
                    facts.add_type(&class.name.name, class.extends.as_ref(), &class.members)
                }
                TopLevelDecl::MixinClass(class) => {
                    facts.add_type(&class.name.name, class.extends.as_ref(), &class.members)
                }
                TopLevelDecl::Mixin(mixin) => {
                    facts.add_type(&mixin.name.name, None, &mixin.members)
                }
                TopLevelDecl::Enum(enumeration) => {
                    facts.add_type(&enumeration.name.name, None, &enumeration.members)
                }
                TopLevelDecl::Extension(extension) => {
                    if let Some(name) = &extension.name {
                        facts.add_type(&name.name, None, &extension.members);
                    }
                }
                TopLevelDecl::ExtensionType(extension) => {
                    facts.add_type(&extension.name.name, None, &extension.members)
                }
                _ => {}
            }
        }
        facts
    }

    fn add_type(&mut self, name: &str, extends: Option<&DartType>, members: &[ClassMember]) {
        let mut facts = TypeFacts {
            superclass: extends.map(|ty| {
                simple_type_name(ty)
                    .map(SuperclassFact::Local)
                    .unwrap_or(SuperclassFact::Unknown)
            }),
            ..TypeFacts::default()
        };
        for member in members {
            match member {
                ClassMember::Constructor(constructor) => {
                    facts.constructors.insert(
                        constructor
                            .constructor_name
                            .as_ref()
                            .map(|name| name.name.clone()),
                        Signature::from_params(&constructor.params, None),
                    );
                }
                ClassMember::Method(method) => {
                    facts.methods.insert(
                        method.name.name.clone(),
                        MethodFact {
                            signature: Signature::from_params(
                                &method.params,
                                method.return_type.as_ref(),
                            ),
                            is_static: method.is_static,
                        },
                    );
                }
                ClassMember::Field(field) => {
                    for declarator in &field.declarators {
                        facts.fields.insert(
                            declarator.name.name.clone(),
                            ValueFact::from_type(field.field_type.as_ref(), field.is_static),
                        );
                    }
                }
                ClassMember::Setter(setter) => {
                    facts.setters.insert(
                        setter.name.name.clone(),
                        ValueFact::from_type(setter.param_type.as_ref(), setter.is_static),
                    );
                }
                _ => {}
            }
        }
        self.types.insert(name.to_string(), facts);
    }

    fn member_signature(&self, type_name: &str, name: &str, is_static: bool) -> Lookup<Signature> {
        let mut current = type_name;
        let mut visited = HashSet::new();
        loop {
            // A cyclic `extends` chain is invalid Dart but still parses.
            if !visited.insert(current) {
                return Lookup::Unknown;
            }
            let Some(facts) = self.types.get(current) else {
                return Lookup::Unknown;
            };
            if let Some(method) = facts.methods.get(name)
                && method.is_static == is_static
            {
                return Lookup::Found(method.signature.clone());
            }
            if let Some(field) = facts.fields.get(name)
                && field.is_static == is_static
            {
                return match &field.signature {
                    Some(signature) => Lookup::Found(signature.clone()),
                    None => Lookup::Absent,
                };
            }
            if is_static {
                return Lookup::Absent;
            }
            match &facts.superclass {
                Some(SuperclassFact::Local(superclass)) => current = superclass,
                Some(SuperclassFact::Unknown) => return Lookup::Unknown,
                None => return Lookup::Absent,
            }
        }
    }

    fn member_value(&self, type_name: &str, name: &str, is_static: bool) -> Lookup<ValueFact> {
        let mut current = type_name;
        let mut visited = HashSet::new();
        loop {
            if !visited.insert(current) {
                return Lookup::Unknown;
            }
            let Some(facts) = self.types.get(current) else {
                return Lookup::Unknown;
            };
            if let Some(field) = facts.fields.get(name)
                && field.is_static == is_static
            {
                return Lookup::Found(field.clone());
            }
            if let Some(setter) = facts.setters.get(name)
                && setter.is_static == is_static
            {
                return Lookup::Found(setter.clone());
            }
            if is_static {
                return Lookup::Absent;
            }
            match &facts.superclass {
                Some(SuperclassFact::Local(superclass)) => current = superclass,
                Some(SuperclassFact::Unknown) => return Lookup::Unknown,
                None => return Lookup::Absent,
            }
        }
    }
}

#[derive(Clone)]
struct LocalValues {
    scopes: Vec<HashMap<String, ValueFact>>,
}

impl LocalValues {
    fn new() -> Self {
        Self {
            scopes: vec![HashMap::new()],
        }
    }

    fn push_scope(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn pop_scope(&mut self) {
        if self.scopes.len() > 1 {
            self.scopes.pop();
        }
    }

    fn declare(&mut self, name: impl Into<String>, fact: ValueFact) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.into(), fact);
        }
    }

    fn lookup(&self, name: &str) -> Option<&ValueFact> {
        self.scopes.iter().rev().find_map(|scope| scope.get(name))
    }
}

struct Collector<'a> {
    diags: Vec<Diagnostic>,
    file: String,
    types: Option<&'a TypeIndex>,
    facts: ProgramFacts,
    locals: LocalTypes,
    local_values: LocalValues,
    enclosing_type: Option<String>,
    current_static: bool,
    returns_function: bool,
}

impl Collector<'_> {
    fn receiver_types(&self) -> ReceiverTypes<'_> {
        ReceiverTypes::new(&self.locals, self.types, self.enclosing_type.as_deref())
    }

    fn type_of_expr(&self, expr: &Expr) -> StaticType {
        if has_ambiguous_prefixed_type(expr, &self.facts) {
            return StaticType::Unknown;
        }
        let resolved = self.receiver_types().of_expr(expr);
        if !matches!(resolved, StaticType::Unknown) {
            return resolved;
        }
        match expr {
            Expr::Ident(identifier) if !self.locals.is_bound(&identifier.name) => self
                .unqualified_value(&identifier.name)
                .map(|fact| fact.static_type.clone())
                .unwrap_or(StaticType::Unknown),
            Expr::Field { object, field, .. } => self
                .field_fact(object, &field.name)
                .map(|fact| fact.static_type.clone())
                .unwrap_or(StaticType::Unknown),
            Expr::NullAssert { operand, .. } => self.type_of_expr(operand).with_nullable(false),
            Expr::As { dart_type, .. } => local_static_type(dart_type),
            other => {
                let _ = other;
                StaticType::Unknown
            }
        }
    }

    fn callable_object(&self, expr: &Expr) -> bool {
        if matches!(expr, Expr::Field { field, .. } if field.name == "call") {
            return false;
        }
        let Some(types) = self.types else {
            return false;
        };
        let StaticType::Other { name, .. } = self.type_of_expr(expr) else {
            return false;
        };
        matches!(
            types.member_lookup(&name, "call"),
            MemberResult::Found(MemberKind::Method)
        )
    }

    fn check(&mut self, expr: &Expr) {
        if self.callable_object(expr) {
            let span = expr.span();
            self.diags.push(Diagnostic::new(
                "implicit-call-tearoffs",
                Severity::Warning,
                "Explicitly tear off the 'call' method.",
                self.file.clone(),
                DiagSpan {
                    start: span.start,
                    end: span.end,
                },
            ));
        }
    }

    fn push_scope(&mut self) {
        self.locals.push_scope();
        self.local_values.push_scope();
    }

    fn pop_scope(&mut self) {
        self.locals.pop_scope();
        self.local_values.pop_scope();
    }

    fn unqualified_value(&self, name: &str) -> Option<ValueFact> {
        if let Some(local) = self.local_values.lookup(name) {
            return Some(local.clone());
        }
        if self.locals.is_bound(name) {
            return None;
        }
        if let Some(enclosing) = &self.enclosing_type {
            match self
                .facts
                .member_value(enclosing, name, self.current_static)
            {
                Lookup::Found(value) => return Some(value),
                Lookup::Unknown => return None,
                Lookup::Absent => {}
            }
        }
        self.facts
            .variables
            .get(name)
            .or_else(|| self.facts.setters.get(name))
            .cloned()
    }

    fn unqualified_signature(&self, name: &str) -> Option<Signature> {
        if let Some(local) = self.local_values.lookup(name) {
            return local.signature.clone();
        }
        if self.locals.is_bound(name) {
            return None;
        }
        if let Some(enclosing) = &self.enclosing_type {
            match self
                .facts
                .member_signature(enclosing, name, self.current_static)
            {
                Lookup::Found(signature) => return Some(signature),
                Lookup::Unknown => return None,
                Lookup::Absent => {}
            }
        }
        self.facts
            .functions
            .get(name)
            .cloned()
            .or_else(|| {
                self.facts
                    .variables
                    .get(name)
                    .and_then(|fact| fact.signature.clone())
            })
            .or_else(|| {
                self.facts
                    .types
                    .get(name)
                    .and_then(|facts| facts.constructors.get(&None))
                    .cloned()
            })
    }

    fn receiver_type_name(&self, object: &Expr) -> Option<(String, bool)> {
        if let Expr::Ident(identifier) = object
            && !self.locals.is_bound(&identifier.name)
            && self.facts.types.contains_key(&identifier.name)
        {
            return Some((identifier.name.clone(), true));
        }
        match self.type_of_expr(object) {
            StaticType::Other { name, .. } if self.facts.types.contains_key(&name) => {
                Some((name, false))
            }
            _ => None,
        }
    }

    fn field_fact(&self, object: &Expr, name: &str) -> Option<ValueFact> {
        let (type_name, static_access) = self.receiver_type_name(object)?;
        match self.facts.member_value(&type_name, name, static_access) {
            Lookup::Found(value) => Some(value),
            Lookup::Absent | Lookup::Unknown => None,
        }
    }

    fn field_signature(&self, object: &Expr, name: &str) -> Option<Signature> {
        let (type_name, static_access) = self.receiver_type_name(object)?;
        match self.facts.member_signature(&type_name, name, static_access) {
            Lookup::Found(signature) => return Some(signature),
            Lookup::Unknown => return None,
            Lookup::Absent => {}
        }
        if static_access {
            return self
                .facts
                .types
                .get(&type_name)?
                .constructors
                .get(&Some(name.to_string()))
                .cloned();
        }
        None
    }

    fn call_signature(&self, callee: &Expr) -> Option<Signature> {
        match callee {
            Expr::Ident(identifier) => self.unqualified_signature(&identifier.name),
            Expr::Field { object, field, .. } => self.field_signature(object, &field.name),
            Expr::GenericInstantiation { target, .. } => self.call_signature(target),
            other => {
                let _ = other;
                None
            }
        }
    }

    fn constructor_signature(
        &self,
        dart_type: &DartType,
        constructor_name: Option<&Identifier>,
    ) -> Option<Signature> {
        let type_name = simple_type_name(dart_type)?;
        self.facts
            .types
            .get(&type_name)?
            .constructors
            .get(&constructor_name.map(|name| name.name.clone()))
            .cloned()
    }

    fn check_args(&mut self, signature: &Signature, args: &ArgList) {
        for (argument, expects_function) in args.positional.iter().zip(&signature.positional) {
            if *expects_function {
                self.check(argument);
            }
        }
        for argument in &args.named {
            if signature
                .named
                .get(&argument.name.name)
                .is_some_and(|expects_function| *expects_function)
            {
                self.check(&argument.value);
            }
        }
    }

    fn target_expects_function(&self, target: &Expr) -> bool {
        match target {
            Expr::Ident(identifier) if !self.locals.is_bound(&identifier.name) => self
                .unqualified_value(&identifier.name)
                .is_some_and(|fact| fact.expects_function),
            Expr::Ident(identifier) => self
                .local_values
                .lookup(&identifier.name)
                .is_some_and(|fact| fact.expects_function),
            Expr::Field { object, field, .. } => self
                .field_fact(object, &field.name)
                .is_some_and(|fact| fact.expects_function),
            _ => false,
        }
    }

    fn value_signature(&self, expr: &Expr) -> Option<Signature> {
        match expr {
            Expr::Ident(identifier) => self.unqualified_signature(&identifier.name),
            Expr::Field { object, field, .. } => self.field_signature(object, &field.name),
            Expr::GenericInstantiation { target, .. } => self.value_signature(target),
            Expr::FuncExpr { params, .. } => Some(Signature::from_params(params, None)),
            other => {
                let _ = other;
                None
            }
        }
    }

    fn visit_params(&mut self, params: &FormalParamList) {
        for param in params
            .positional
            .iter()
            .chain(&params.optional_positional)
            .chain(&params.named)
        {
            if formal_expects_function(param)
                && let Some(default) = &param.default_value
            {
                self.check(default);
            }
            self.visit_formal_param(param);
            let mut fact = ValueFact::from_type(param.param_type.as_ref(), false);
            fact.expects_function = formal_expects_function(param);
            fact.signature = Signature::from_formal(param);
            self.locals
                .declare(param.name.name.clone(), fact.static_type.clone());
            self.local_values.declare(param.name.name.clone(), fact);
        }
    }

    fn function(
        &mut self,
        params: &FormalParamList,
        body: Option<&FunctionBody>,
        signature: &Signature,
        fresh: bool,
    ) {
        let saved_locals = fresh.then(|| std::mem::replace(&mut self.locals, LocalTypes::new()));
        let saved_values =
            fresh.then(|| std::mem::replace(&mut self.local_values, LocalValues::new()));
        if !fresh {
            self.push_scope();
        }
        let previous_return =
            std::mem::replace(&mut self.returns_function, signature.returns_function);
        self.visit_params(params);
        if let Some(body) = body {
            self.body(body);
        }
        self.returns_function = previous_return;
        if let Some(saved) = saved_locals {
            self.locals = saved;
            self.local_values = saved_values.expect("fresh local-value scope");
        } else {
            self.pop_scope();
        }
    }

    fn body(&mut self, body: &FunctionBody) {
        match body {
            FunctionBody::Block(block) => self.statements(&block.stmts),
            FunctionBody::Arrow(expr, _) => {
                if self.returns_function {
                    self.check(expr);
                }
                self.visit_expr(expr);
            }
            FunctionBody::Native(_, _) => {}
        }
    }

    fn statements(&mut self, statements: &[Stmt]) {
        for statement in statements {
            if let Stmt::LocalFunc(function) = statement {
                self.declare_local_function(function);
            }
        }
        for statement in statements {
            self.visit_stmt(statement);
        }
    }

    fn declare_local_function(&mut self, function: &LocalFuncDecl) {
        let signature = Signature::from_params(&function.params, function.return_type.as_ref());
        self.locals
            .declare(function.name.name.clone(), StaticType::Unknown);
        self.local_values.declare(
            function.name.name.clone(),
            ValueFact {
                static_type: StaticType::Unknown,
                expects_function: true,
                signature: Some(signature),
                is_static: false,
            },
        );
    }

    fn constructor_initializer(&mut self, initializer: &ConstructorInitializer) {
        match initializer {
            ConstructorInitializer::SuperCall {
                call_name, args, ..
            } => {
                let signature = self
                    .enclosing_type
                    .as_ref()
                    .and_then(|name| self.facts.types.get(name))
                    .and_then(|facts| match facts.superclass.as_ref()? {
                        SuperclassFact::Local(name) => self.facts.types.get(name),
                        SuperclassFact::Unknown => None,
                    })
                    .and_then(|facts| {
                        facts
                            .constructors
                            .get(&call_name.as_ref().map(|name| name.name.clone()))
                    })
                    .cloned();
                if let Some(signature) = signature {
                    self.check_args(&signature, args);
                }
                self.visit_args(args);
            }
            ConstructorInitializer::ThisCall {
                call_name, args, ..
            } => {
                let signature = self
                    .enclosing_type
                    .as_ref()
                    .and_then(|name| self.facts.types.get(name))
                    .and_then(|facts| {
                        facts
                            .constructors
                            .get(&call_name.as_ref().map(|name| name.name.clone()))
                    })
                    .cloned();
                if let Some(signature) = signature {
                    self.check_args(&signature, args);
                }
                self.visit_args(args);
            }
            ConstructorInitializer::FieldInit { field, value, .. } => {
                if self
                    .enclosing_type
                    .as_ref()
                    .and_then(|name| self.facts.types.get(name))
                    .and_then(|facts| facts.fields.get(&field.name))
                    .is_some_and(|fact| fact.expects_function)
                {
                    self.check(value);
                }
                self.visit_expr(value);
            }
            ConstructorInitializer::Assert {
                condition, message, ..
            } => {
                self.visit_expr(condition);
                if let Some(message) = message {
                    self.visit_expr(message);
                }
            }
        }
    }

    fn visit_args(&mut self, args: &ArgList) {
        for argument in &args.positional {
            self.visit_expr(argument);
        }
        for argument in &args.named {
            self.visit_expr(&argument.value);
        }
    }

    fn with_enclosing_type(&mut self, name: Option<&str>, members: &[ClassMember]) {
        let previous = std::mem::replace(&mut self.enclosing_type, name.map(str::to_string));
        for member in members {
            self.visit_class_member(member);
        }
        self.enclosing_type = previous;
    }

    fn local_var(&mut self, declaration: &LocalVarDecl) {
        for declarator in &declaration.declarators {
            if declaration.var_type.as_ref().is_some_and(is_function_type)
                && let Some(initializer) = &declarator.initializer
            {
                self.check(initializer);
            }
            if let Some(initializer) = &declarator.initializer {
                self.visit_expr(initializer);
            }

            let static_type = declaration
                .var_type
                .as_ref()
                .map(local_static_type)
                .or_else(|| {
                    declarator
                        .initializer
                        .as_ref()
                        .map(|initializer| self.type_of_expr(initializer))
                })
                .unwrap_or(StaticType::Unknown);
            let signature = declaration
                .var_type
                .as_ref()
                .and_then(signature_from_type)
                .or_else(|| {
                    declarator
                        .initializer
                        .as_ref()
                        .and_then(|initializer| self.value_signature(initializer))
                });
            self.locals
                .declare(declarator.name.name.clone(), static_type.clone());
            self.local_values.declare(
                declarator.name.name.clone(),
                ValueFact {
                    static_type,
                    expects_function: declaration.var_type.as_ref().is_some_and(is_function_type),
                    signature,
                    is_static: false,
                },
            );
        }
    }

    fn for_stmt(&mut self, node: &ForStmt) {
        self.push_scope();
        if let Some(init) = &node.init {
            match init {
                ForInit::VarDecl(declaration) => self.local_var(declaration),
                ForInit::ForIn {
                    var_type,
                    name,
                    iterable,
                    ..
                } => {
                    self.visit_expr(iterable);
                    let fact = ValueFact::from_type(var_type.as_ref(), false);
                    self.locals
                        .declare(name.name.clone(), fact.static_type.clone());
                    self.local_values.declare(name.name.clone(), fact);
                }
                ForInit::PatternForIn { pattern, iterable } => {
                    self.visit_expr(iterable);
                    self.visit_pattern(pattern);
                    self.bind_pattern_values(pattern);
                }
                ForInit::Exprs(expressions) => {
                    for expression in expressions {
                        self.visit_expr(expression);
                    }
                }
            }
        }
        if let Some(condition) = &node.condition {
            self.visit_expr(condition);
        }
        for update in &node.update {
            self.visit_expr(update);
        }
        self.visit_stmt(&node.body);
        self.pop_scope();
    }

    fn bind_pattern_values(&mut self, pattern: &Pattern) {
        match pattern {
            Pattern::Variable { type_, name, .. } => {
                let fact = ValueFact::from_type(type_.as_ref(), false);
                self.locals
                    .declare(name.name.clone(), fact.static_type.clone());
                self.local_values.declare(name.name.clone(), fact);
            }
            Pattern::List(list) => {
                for element in &list.elements {
                    match element {
                        ListPatternElement::Pattern(pattern)
                        | ListPatternElement::Rest(Some(pattern), _) => {
                            self.bind_pattern_values(pattern)
                        }
                        ListPatternElement::Rest(None, _) => {}
                    }
                }
            }
            Pattern::Record(record) => {
                for field in &record.fields {
                    self.bind_pattern_values(&field.pattern);
                }
            }
            Pattern::Map(map) => {
                for entry in &map.entries {
                    self.bind_pattern_values(&entry.pattern);
                }
            }
            Pattern::Object(object) => {
                for field in &object.fields {
                    if let Some(pattern) = &field.pattern {
                        self.bind_pattern_values(pattern);
                    }
                }
            }
            Pattern::LogicalAnd { left, right, .. } | Pattern::LogicalOr { left, right, .. } => {
                self.bind_pattern_values(left);
                self.bind_pattern_values(right);
            }
            Pattern::Cast { inner, .. }
            | Pattern::NullCheck { inner, .. }
            | Pattern::NullAssert { inner, .. }
            | Pattern::ParenPattern { inner, .. } => self.bind_pattern_values(inner),
            _ => {}
        }
    }

    fn try_catch(&mut self, node: &TryCatchStmt) {
        self.push_scope();
        self.statements(&node.body.stmts);
        self.pop_scope();
        for catch in &node.catches {
            self.push_scope();
            if let Some(variable) = &catch.exception_var {
                let fact = ValueFact::from_type(catch.exception_type.as_ref(), false);
                self.locals
                    .declare(variable.name.clone(), fact.static_type.clone());
                self.local_values.declare(variable.name.clone(), fact);
            }
            if let Some(variable) = &catch.stack_trace_var {
                let fact = ValueFact::from_type(None, false);
                self.locals
                    .declare(variable.name.clone(), fact.static_type.clone());
                self.local_values.declare(variable.name.clone(), fact);
            }
            self.statements(&catch.body.stmts);
            self.pop_scope();
        }
        if let Some(finally) = &node.finally {
            self.push_scope();
            self.statements(&finally.stmts);
            self.pop_scope();
        }
    }
}

impl Visitor for Collector<'_> {
    fn visit_program(&mut self, node: &Program) {
        for declaration in &node.declarations {
            self.visit_top_level_decl(declaration);
        }
    }

    fn visit_top_level_decl(&mut self, node: &TopLevelDecl) {
        match node {
            TopLevelDecl::Class(class) => {
                self.with_enclosing_type(Some(&class.name.name), &class.members)
            }
            TopLevelDecl::MixinClass(class) => {
                self.with_enclosing_type(Some(&class.name.name), &class.members)
            }
            TopLevelDecl::Mixin(mixin) => {
                self.with_enclosing_type(Some(&mixin.name.name), &mixin.members)
            }
            TopLevelDecl::Enum(enumeration) => {
                for variant in &enumeration.variants {
                    if let Some(args) = &variant.args {
                        if let Some(signature) = self
                            .facts
                            .types
                            .get(&enumeration.name.name)
                            .and_then(|facts| {
                                facts.constructors.get(
                                    &variant
                                        .constructor_name
                                        .as_ref()
                                        .map(|name| name.name.clone()),
                                )
                            })
                            .cloned()
                        {
                            self.check_args(&signature, args);
                        }
                        self.visit_args(args);
                    }
                }
                self.with_enclosing_type(Some(&enumeration.name.name), &enumeration.members);
            }
            TopLevelDecl::Extension(extension) => self.with_enclosing_type(
                extension.name.as_ref().map(|name| name.name.as_str()),
                &extension.members,
            ),
            TopLevelDecl::ExtensionType(extension) => {
                self.with_enclosing_type(Some(&extension.name.name), &extension.members)
            }
            TopLevelDecl::Function(function) => self.visit_function_decl(function),
            TopLevelDecl::Variable(variable) => {
                for declarator in &variable.declarators {
                    if variable.var_type.as_ref().is_some_and(is_function_type)
                        && let Some(initializer) = &declarator.initializer
                    {
                        self.check(initializer);
                    }
                    if let Some(initializer) = &declarator.initializer {
                        self.visit_expr(initializer);
                    }
                }
            }
            _ => {}
        }
    }

    fn visit_function_decl(&mut self, node: &FunctionDecl) {
        let signature = Signature::from_params(&node.params, node.return_type.as_ref());
        self.function(&node.params, node.body.as_ref(), &signature, true);
    }

    fn visit_class_member(&mut self, node: &ClassMember) {
        match node {
            ClassMember::Field(field) => self.visit_field_decl(field),
            ClassMember::Constructor(constructor) => self.visit_constructor_decl(constructor),
            ClassMember::Method(method) => self.visit_method_decl(method),
            ClassMember::Getter(getter) => self.visit_getter_decl(getter),
            ClassMember::Setter(setter) => self.visit_setter_decl(setter),
            ClassMember::Operator(operator) => {
                let signature =
                    Signature::from_params(&operator.params, operator.return_type.as_ref());
                let previous_static = std::mem::replace(&mut self.current_static, false);
                self.function(&operator.params, operator.body.as_ref(), &signature, true);
                self.current_static = previous_static;
            }
            ClassMember::Error(_) => {}
        }
    }

    fn visit_field_decl(&mut self, node: &FieldDecl) {
        let previous_static = std::mem::replace(&mut self.current_static, node.is_static);
        for declarator in &node.declarators {
            if node.field_type.as_ref().is_some_and(is_function_type)
                && let Some(initializer) = &declarator.initializer
            {
                self.check(initializer);
            }
            if let Some(initializer) = &declarator.initializer {
                self.visit_expr(initializer);
            }
        }
        self.current_static = previous_static;
    }

    fn visit_method_decl(&mut self, node: &MethodDecl) {
        let signature = Signature::from_params(&node.params, node.return_type.as_ref());
        let previous_static = std::mem::replace(&mut self.current_static, node.is_static);
        self.function(&node.params, node.body.as_ref(), &signature, true);
        self.current_static = previous_static;
    }

    fn visit_constructor_decl(&mut self, node: &ConstructorDecl) {
        let saved_locals = std::mem::replace(&mut self.locals, LocalTypes::new());
        let saved_values = std::mem::replace(&mut self.local_values, LocalValues::new());
        let previous_static = std::mem::replace(&mut self.current_static, false);
        let previous_return = std::mem::replace(&mut self.returns_function, false);
        self.visit_params(&node.params);
        for initializer in &node.initializers {
            self.constructor_initializer(initializer);
        }
        if let Some(body) = &node.body {
            self.body(body);
        }
        self.returns_function = previous_return;
        self.current_static = previous_static;
        self.locals = saved_locals;
        self.local_values = saved_values;
    }

    fn visit_getter_decl(&mut self, node: &GetterDecl) {
        let saved_locals = std::mem::replace(&mut self.locals, LocalTypes::new());
        let saved_values = std::mem::replace(&mut self.local_values, LocalValues::new());
        let previous_static = std::mem::replace(&mut self.current_static, node.is_static);
        let previous_return = std::mem::replace(
            &mut self.returns_function,
            node.return_type.as_ref().is_some_and(is_function_type),
        );
        if let Some(body) = &node.body {
            self.body(body);
        }
        self.returns_function = previous_return;
        self.current_static = previous_static;
        self.locals = saved_locals;
        self.local_values = saved_values;
    }

    fn visit_setter_decl(&mut self, node: &SetterDecl) {
        let saved_locals = std::mem::replace(&mut self.locals, LocalTypes::new());
        let saved_values = std::mem::replace(&mut self.local_values, LocalValues::new());
        let previous_static = std::mem::replace(&mut self.current_static, node.is_static);
        let fact = ValueFact::from_type(node.param_type.as_ref(), false);
        self.locals
            .declare(node.param.name.clone(), fact.static_type.clone());
        self.local_values.declare(node.param.name.clone(), fact);
        if let Some(body) = &node.body {
            self.body(body);
        }
        self.current_static = previous_static;
        self.locals = saved_locals;
        self.local_values = saved_values;
    }

    fn visit_stmt(&mut self, node: &Stmt) {
        match node {
            Stmt::LocalVar(declaration) => self.local_var(declaration),
            Stmt::PatternDecl(declaration) => {
                self.visit_expr(&declaration.init);
                self.visit_pattern(&declaration.pattern);
                self.bind_pattern_values(&declaration.pattern);
            }
            Stmt::Block(block) => {
                self.push_scope();
                self.statements(&block.stmts);
                self.pop_scope();
            }
            Stmt::If(if_statement) => {
                if let IfCondition::Case(scrutinee, pattern, guard) = &if_statement.condition {
                    self.visit_expr(scrutinee);
                    self.push_scope();
                    self.visit_pattern(pattern);
                    self.bind_pattern_values(pattern);
                    if let Some(guard) = guard {
                        self.visit_expr(guard);
                    }
                    self.visit_stmt(&if_statement.then_branch);
                    self.pop_scope();
                    if let Some(else_branch) = &if_statement.else_branch {
                        self.visit_stmt(else_branch);
                    }
                } else {
                    walk_stmt(self, node);
                }
            }
            Stmt::For(for_statement) => self.for_stmt(for_statement),
            Stmt::Switch(switch) => {
                self.visit_expr(&switch.subject);
                for case in &switch.cases {
                    self.push_scope();
                    for kind in &case.cases {
                        if let SwitchCaseKind::Pattern(pattern, guard) = kind {
                            self.visit_pattern(pattern);
                            self.locals.bind_pattern(pattern);
                            self.bind_pattern_values(pattern);
                            if let Some(guard) = guard.as_ref() {
                                self.visit_expr(guard);
                            }
                        }
                    }
                    self.statements(&case.body);
                    self.pop_scope();
                }
            }
            Stmt::TryCatch(try_catch) => self.try_catch(try_catch),
            Stmt::LocalFunc(function) => {
                let signature =
                    Signature::from_params(&function.params, function.return_type.as_ref());
                self.function(&function.params, Some(&function.body), &signature, false);
            }
            Stmt::Return(return_statement) => {
                if let Some(value) = &return_statement.value {
                    if self.returns_function {
                        self.check(value);
                    }
                    self.visit_expr(value);
                }
            }
            other => walk_stmt(self, other),
        }
    }

    fn visit_expr(&mut self, node: &Expr) {
        match node {
            Expr::Call { callee, args, .. } => {
                if let Some(signature) = self.call_signature(callee) {
                    self.check_args(&signature, args);
                }
                walk_expr(self, node);
            }
            Expr::New {
                dart_type,
                constructor_name,
                args,
                ..
            } => {
                if let Some(signature) =
                    self.constructor_signature(dart_type, constructor_name.as_ref())
                {
                    self.check_args(&signature, args);
                }
                walk_expr(self, node);
            }
            Expr::Assign {
                target, op, value, ..
            } => {
                if matches!(op, AssignOp::Eq) && self.target_expects_function(target) {
                    self.check(value);
                }
                self.visit_expr(target);
                self.visit_expr(value);
                if let Expr::Ident(identifier) = target.as_ref() {
                    let static_type = self.type_of_expr(value);
                    let signature = self.value_signature(value);
                    self.locals.reassign(&identifier.name, static_type.clone());
                    if let Some(fact) = self
                        .local_values
                        .scopes
                        .iter_mut()
                        .rev()
                        .find_map(|scope| scope.get_mut(&identifier.name))
                    {
                        if fact.static_type != static_type {
                            fact.static_type = StaticType::Unknown;
                        }
                        fact.signature = signature;
                    }
                }
            }
            Expr::FuncExpr { params, body, .. } => {
                let signature = Signature::from_params(params, None);
                self.function(params, Some(body), &signature, false);
            }
            other => walk_expr(self, other),
        }
    }
}

struct SemanticCollector {
    diags: Vec<Diagnostic>,
    file: String,
}

impl SemanticRuleVisitor for SemanticCollector {
    fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
        if matches!(node, Expr::Field { field, .. } if field.name == "call") {
            return;
        }
        let Some(expected) = state.expected() else {
            return;
        };
        if !matches!(expected, ResolvedType::Function { .. })
            && !expected.interface("dart:core", "Function")
        {
            return;
        }
        let receiver = state.infer(node);
        if matches!(
            receiver,
            ResolvedType::Function { .. } | ResolvedType::Unknown | ResolvedType::Dynamic
        ) || !state
            .signatures
            .resolved_member_facts(&receiver, "call", &state.model)
            .is_some_and(|facts| {
                facts
                    .iter()
                    .any(|fact| fact.kind == SemanticMemberKind::Method && !fact.is_static)
            })
        {
            return;
        }
        let span = node.span();
        self.diags.push(Diagnostic::new(
            "implicit-call-tearoffs",
            Severity::Warning,
            "Explicitly tear off the 'call' method.",
            self.file.clone(),
            DiagSpan {
                start: span.start,
                end: span.end,
            },
        ));
    }
}

fn formal_expects_function(param: &FormalParam) -> bool {
    param.function_params.is_some() || param.param_type.as_ref().is_some_and(is_function_type)
}

fn signature_from_type(ty: &DartType) -> Option<Signature> {
    match ty {
        DartType::Function(function) => Some(Signature::from_function_type(function)),
        _ => None,
    }
}

fn is_function_type(ty: &DartType) -> bool {
    matches!(ty, DartType::Function(_))
        || matches!(ty, DartType::Named(named) if named.segments.last().is_some_and(|name| name.name == "Function"))
}

fn local_static_type(ty: &DartType) -> StaticType {
    match ty {
        DartType::Named(named) if named.segments.len() > 1 => StaticType::Unknown,
        _ => StaticType::from_dart_type(ty),
    }
}

fn simple_type_name(ty: &DartType) -> Option<String> {
    match ty {
        DartType::Named(named) if named.segments.len() == 1 => {
            named.segments.first().map(|name| name.name.clone())
        }
        _ => None,
    }
}

fn has_ambiguous_prefixed_type(expr: &Expr, facts: &ProgramFacts) -> bool {
    match expr {
        Expr::New { dart_type, .. } | Expr::As { dart_type, .. } => {
            matches!(dart_type, DartType::Named(named) if named.segments.len() > 1)
        }
        Expr::Call { callee, .. } => match callee.as_ref() {
            Expr::Field { object, field, .. } => {
                matches!(object.as_ref(), Expr::Ident(prefix)
                    if !facts.types.contains_key(&prefix.name)
                        && facts.types.contains_key(&field.name))
            }
            _ => false,
        },
        _ => false,
    }
}
