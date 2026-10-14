//! Flags `final` declarations whose initializers are all constant expressions.
//!
//! A `final` local variable, top-level variable, or static field initialized
//! with a constant expression can be declared `const` instead, which documents
//! the intent and lets the value be canonicalized at compile time. List, map,
//! and set literals only count when written with an explicit `const`, since a
//! plain literal creates a fresh mutable collection. Instance fields are never
//! reported, and every declarator in the declaration must qualify.
//!
//! Constness is decided from this file alone: literals (including records with
//! constant fields), explicit `const` creations, operators over constants, and
//! references to `const` variables, static `const` fields, and enum values
//! declared in this file. Imported names, calls, and type tests that cannot be
//! proven constant here keep the declaration quiet.
//!
//! ```dart
//! final o = const <int>[]; // flagged
//! const p = <int>[]; // fine
//! ```
//!
//! Adopted from the official `prefer_const_declarations` lint.

use std::collections::HashMap;

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{
    Visitor, bound_names, walk_class_member, walk_expr, walk_formal_param, walk_function_decl,
    walk_local_var, walk_pattern, walk_stmt, walk_top_level_decl,
};

#[derive(Default)]
pub struct PreferConstDeclarations;

impl Rule for PreferConstDeclarations {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut top_level = HashMap::new();
        let mut types = HashMap::new();
        let mut declarations = HashMap::new();
        for declaration in &program.declarations {
            collect_top_level(declaration, &mut top_level, &mut types, &mut declarations);
        }
        let mut checker = Checker {
            ctx,
            top_level,
            types,
            declarations,
            members: None,
            scopes: Vec::new(),
            diagnostics: Vec::new(),
        };
        checker.visit_program(program);
        checker.diagnostics
    }
}

/// Names declared by one type: `true` for static `const` fields and enum values.
type Members<'p> = HashMap<&'p str, bool>;

/// Identifies a type declaration by its address in the visited program.
fn declaration_key(declaration: &TopLevelDecl) -> usize {
    std::ptr::from_ref(declaration).addr()
}

fn collect_top_level<'p>(
    declaration: &'p TopLevelDecl,
    top_level: &mut HashMap<&'p str, bool>,
    types: &mut HashMap<&'p str, usize>,
    declarations: &mut HashMap<usize, Members<'p>>,
) {
    let (name, members) = match declaration {
        TopLevelDecl::Variable(variable) => {
            for declarator in &variable.declarators {
                top_level.insert(&declarator.name.name, variable.keyword == VarKeyword::Const);
            }
            return;
        }
        TopLevelDecl::Function(function) => {
            top_level.insert(&function.name.name, false);
            return;
        }
        TopLevelDecl::ClassTypeAlias(alias) => {
            top_level.insert(&alias.name.name, false);
            return;
        }
        TopLevelDecl::TypeAlias(alias) => {
            top_level.insert(&alias.name.name, false);
            return;
        }
        TopLevelDecl::Error(_) => return,
        TopLevelDecl::Class(class) => (Some(&class.name), type_members(&class.members)),
        TopLevelDecl::Mixin(mixin) => (Some(&mixin.name), type_members(&mixin.members)),
        TopLevelDecl::MixinClass(class) => (Some(&class.name), type_members(&class.members)),
        TopLevelDecl::Extension(extension) => {
            (extension.name.as_ref(), type_members(&extension.members))
        }
        TopLevelDecl::ExtensionType(extension) => {
            let mut members = type_members(&extension.members);
            members.insert(&extension.representation.field_name.name, false);
            (Some(&extension.name), members)
        }
        TopLevelDecl::Enum(enumeration) => {
            let mut members = type_members(&enumeration.members);
            for variant in &enumeration.variants {
                members.insert(&variant.name.name, true);
            }
            members.insert("values", false);
            (Some(&enumeration.name), members)
        }
    };
    let key = declaration_key(declaration);
    if let Some(name) = name {
        top_level.insert(&name.name, false);
        types.insert(&name.name, key);
    }
    declarations.insert(key, members);
}

fn type_members(members: &[ClassMember]) -> Members<'_> {
    let mut names = HashMap::new();
    for member in members {
        match member {
            ClassMember::Field(field) => {
                let is_const = field.is_static && field.keyword == VarKeyword::Const;
                for declarator in &field.declarators {
                    names.insert(declarator.name.name.as_str(), is_const);
                }
            }
            ClassMember::Method(method) => {
                names.insert(method.name.name.as_str(), false);
            }
            ClassMember::Getter(getter) => {
                names.insert(getter.name.name.as_str(), false);
            }
            ClassMember::Setter(setter) => {
                names.insert(setter.name.name.as_str(), false);
            }
            ClassMember::Constructor(_) | ClassMember::Operator(_) | ClassMember::Error(_) => {}
        }
    }
    names
}

struct Checker<'a, 'ctx, 'p> {
    ctx: &'a AnalyzeContext<'ctx>,
    /// Top-level names of this file; `true` for `const` variables.
    top_level: HashMap<&'p str, bool>,
    /// Named type declarations of this file, by name.
    types: HashMap<&'p str, usize>,
    /// Members of every type declaration of this file, by declaration key.
    declarations: HashMap<usize, Members<'p>>,
    /// Key of the enclosing type declaration, if any.
    members: Option<usize>,
    /// Local bindings, innermost last; `true` for `const` locals.
    scopes: Vec<HashMap<String, bool>>,
    diagnostics: Vec<Diagnostic>,
}

impl Checker<'_, '_, '_> {
    fn scoped(&mut self, walk: impl FnOnce(&mut Self)) {
        self.scopes.push(HashMap::new());
        walk(self);
        self.scopes.pop();
    }

    fn declare(&mut self, name: &str, is_const: bool) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name.to_string(), is_const);
        }
    }

    fn local(&self, name: &str) -> Option<bool> {
        self.scopes
            .iter()
            .rev()
            .find_map(|scope| scope.get(name).copied())
    }

    fn enclosing_members(&self) -> Option<&Members<'_>> {
        self.members.and_then(|key| self.declarations.get(&key))
    }

    /// Whether `name` provably denotes a constant variable from this file.
    fn constant_name(&self, name: &str) -> bool {
        if let Some(is_const) = self.local(name) {
            return is_const;
        }
        if let Some(is_const) = self
            .enclosing_members()
            .and_then(|members| members.get(name))
        {
            return *is_const;
        }
        self.top_level.get(name).copied().unwrap_or(false)
    }

    /// The members of the type declared in this file that `name` denotes, unless
    /// a local or member binding shadows it.
    fn type_members(&self, name: &str) -> Option<&Members<'_>> {
        let shadowed = self.local(name).is_some()
            || self
                .enclosing_members()
                .is_some_and(|members| members.contains_key(name));
        if shadowed {
            return None;
        }
        self.types
            .get(name)
            .and_then(|key| self.declarations.get(key))
    }

    fn is_constant(&self, expr: &Expr) -> bool {
        let mut constant = ConstantExpr {
            names: Some(self),
            valid: true,
        };
        constant.visit_expr(expr);
        constant.valid
    }

    /// Whether `owner.field` names a static `const` field or enum value of a
    /// type declared in this file.
    fn constant_member(&self, owner: &str, field: &str) -> bool {
        self.type_members(owner)
            .and_then(|members| members.get(field))
            .copied()
            .unwrap_or(false)
    }

    fn check(&mut self, keyword: VarKeyword, declarators: &[VarDeclarator], span: Span) {
        if keyword != VarKeyword::Final {
            return;
        }
        let all_constant = !declarators.is_empty()
            && declarators.iter().all(|declarator| {
                declarator
                    .initializer
                    .as_ref()
                    .is_some_and(|initializer| self.is_constant(initializer))
            });
        if !all_constant {
            return;
        }
        let start =
            final_keyword_start(self.ctx.source, span.start, declarators[0].name.span.start)
                .unwrap_or(span.start);
        self.diagnostics.push(self.ctx.diagnostic(
            "prefer-const-declarations",
            "Use 'const' for final variables initialized to a constant value.",
            Span {
                start,
                end: span.end
                    - usize::from(self.ctx.source.as_bytes().get(span.end - 1) == Some(&b';')),
            },
        ));
    }
}

impl Visitor for Checker<'_, '_, '_> {
    fn visit_top_level_decl(&mut self, node: &TopLevelDecl) {
        if let TopLevelDecl::Variable(variable) = node {
            self.check(variable.keyword, &variable.declarators, variable.span);
        }
        let key = declaration_key(node);
        let enclosing = self.declarations.contains_key(&key).then_some(key);
        let saved = std::mem::replace(&mut self.members, enclosing);
        walk_top_level_decl(self, node);
        self.members = saved;
    }

    fn visit_function_decl(&mut self, node: &FunctionDecl) {
        self.scoped(|this| walk_function_decl(this, node));
    }

    fn visit_class_member(&mut self, node: &ClassMember) {
        if let ClassMember::Field(field) = node
            && field.is_static
        {
            self.check(field.keyword, &field.declarators, field.span);
        }
        self.scoped(|this| walk_class_member(this, node));
    }

    fn visit_formal_param(&mut self, node: &FormalParam) {
        self.declare(&node.name.name, false);
        walk_formal_param(self, node);
    }

    fn visit_local_var(&mut self, node: &LocalVarDecl) {
        walk_local_var(self, node);
        for declarator in &node.declarators {
            self.declare(&declarator.name.name, node.keyword == VarKeyword::Const);
        }
    }

    fn visit_pattern(&mut self, node: &Pattern) {
        for name in bound_names(node) {
            self.declare(&name.name, false);
        }
        walk_pattern(self, node);
    }

    fn visit_stmt(&mut self, node: &Stmt) {
        match node {
            Stmt::LocalVar(local) => {
                self.check(local.keyword, &local.declarators, local.span);
                walk_stmt(self, node);
            }
            Stmt::LocalFunc(function) => {
                self.declare(&function.name.name, false);
                self.scoped(|this| walk_stmt(this, node));
            }
            Stmt::For(for_stmt) => self.scoped(|this| {
                if let Some(ForInit::ForIn { name, .. }) = &for_stmt.header.init {
                    this.declare(&name.name, false);
                }
                walk_stmt(this, node);
            }),
            Stmt::TryCatch(try_catch) => self.scoped(|this| {
                for catch in &try_catch.catches {
                    for name in [&catch.exception_var, &catch.stack_trace_var]
                        .into_iter()
                        .flatten()
                    {
                        this.declare(&name.name, false);
                    }
                }
                walk_stmt(this, node);
            }),
            Stmt::Block(_)
            | Stmt::If(_)
            | Stmt::While(_)
            | Stmt::DoWhile(_)
            | Stmt::Switch(_)
            | Stmt::Labeled(_) => self.scoped(|this| walk_stmt(this, node)),
            _ => walk_stmt(self, node),
        }
    }

    fn visit_expr(&mut self, node: &Expr) {
        if matches!(node, Expr::FuncExpr { .. } | Expr::Switch { .. }) {
            self.scoped(|this| walk_expr(this, node));
        } else {
            walk_expr(self, node);
        }
    }
}

/// Whether the constant evaluator would reject `left ~/ 0` or `left % 0`.
fn divides_by_zero(op: BinaryOp, right: &Expr) -> bool {
    matches!(op, BinaryOp::IntDiv | BinaryOp::Mod)
        && matches!(right, Expr::IntLit { value, .. } if value.parse::<u64>() == Ok(0))
}

/// Whether every interpolation of a string literal is a literal of a type a
/// constant string may interpolate (`num`, `bool`, `String`, `Null`), possibly
/// combined with operators. Names are rejected because their types are not
/// known here.
fn primitive_interpolations(string: &StringLitNode) -> bool {
    string.interpolations.iter().all(|interpolation| {
        let mut primitive = ConstantExpr {
            names: None,
            valid: true,
        };
        primitive.visit_expr(&interpolation.expr);
        primitive.valid
    })
}

/// Decides whether an expression is provably constant. With `names` it accepts
/// references to constants of this file; without, only primitive literals and
/// operators over them (the operands a constant string may interpolate).
struct ConstantExpr<'c, 'a, 'ctx, 'p> {
    names: Option<&'c Checker<'a, 'ctx, 'p>>,
    valid: bool,
}

impl Visitor for ConstantExpr<'_, '_, '_, '_> {
    fn visit_expr(&mut self, node: &Expr) {
        if !self.valid {
            return;
        }
        self.valid = match node {
            Expr::IntLit { .. }
            | Expr::DoubleLit { .. }
            | Expr::BoolLit { .. }
            | Expr::NullLit { .. } => true,
            Expr::SymbolLit { .. } => self.names.is_some(),
            Expr::StringLit(string) => primitive_interpolations(string),
            Expr::List { is_const, .. }
            | Expr::Map { is_const, .. }
            | Expr::Set { is_const, .. }
            | Expr::New { is_const, .. } => self.names.is_some() && *is_const,
            Expr::Record { .. } if self.names.is_some() => {
                walk_expr(self, node);
                return;
            }
            Expr::Ident(identifier) => self
                .names
                .is_some_and(|names| names.constant_name(&identifier.name)),
            Expr::Field {
                object,
                field,
                is_null_safe: false,
                ..
            } => match (self.names, object.as_ref()) {
                (Some(names), Expr::Ident(owner)) => {
                    names.constant_member(&owner.name, &field.name)
                }
                _ => false,
            },
            Expr::Unary {
                op: UnaryOp::Minus | UnaryOp::Bang | UnaryOp::Tilde,
                ..
            }
            | Expr::Conditional { .. } => {
                walk_expr(self, node);
                return;
            }
            Expr::Binary { op, right, .. } => {
                if !divides_by_zero(*op, right) {
                    walk_expr(self, node);
                    return;
                }
                false
            }
            _ => false,
        };
    }
}

/// The start of the last whole-word `final` keyword in `source[start..end]`.
fn final_keyword_start(source: &str, start: usize, end: usize) -> Option<usize> {
    let text = source.get(start..end)?;
    let is_word = |c: char| c.is_alphanumeric() || c == '_' || c == '$';
    text.match_indices("final")
        .filter(|(index, _)| {
            !text[..*index].chars().next_back().is_some_and(is_word)
                && !text[index + "final".len()..]
                    .chars()
                    .next()
                    .is_some_and(is_word)
        })
        .last()
        .map(|(index, _)| start + index)
}
