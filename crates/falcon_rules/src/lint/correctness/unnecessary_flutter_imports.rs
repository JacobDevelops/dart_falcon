//! Disallow unnecessary Flutter framework and `dart:async` imports.
//!
//! Uses each import's export namespace, including re-exports and combinators.
//! A used import is removable only when a retained import provides all its used
//! names and at least one additional used name, matching `unnecessary_import`.
//! Equal coverage is retained, including duplicate imports. Unknown namespaces
//! conservatively keep the import.
//! Resolved extension members keep the import providing their extension;
//! unresolved member lookups keep every import, including prefixed imports.
//! Ordinary instance/static members need no import beyond their receiver's type.
//! Dynamic dispatch cannot apply an extension and does not retain imports.

use std::collections::HashSet;

use falcon_analyze::MemberResult;
use falcon_analyze::{
    AnalyzeContext, CascadeAssignment, DeclarationIdentity, MemberSelectorKind, QualifiedName,
    ResolvedType, ResolvedTypeView, Rule, RulePreparation, SemanticModel, TypeParameterScope,
};
use falcon_analyze::{SemanticRuleVisitor, SemanticState};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::visit::{
    Visitor, walk_annotation, walk_class_decl, walk_class_type_alias, walk_const_pattern_value,
    walk_dart_type, walk_enum_decl, walk_expr, walk_expr_chain, walk_extension_decl,
    walk_extension_type, walk_formal_param, walk_function_decl, walk_function_type,
    walk_local_func_decl, walk_method_decl, walk_mixin_class, walk_mixin_decl, walk_pattern,
    walk_type_alias_decl,
};
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct UnnecessaryFlutterImports;

impl Rule for UnnecessaryFlutterImports {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        if !program.imports().any(import_targets_framework) {
            return RulePreparation::Diagnostics(Vec::new());
        }
        RulePreparation::Semantic(Box::new(Usage {
            program,
            ctx,
            names: Names {
                prefixes: program
                    .imports()
                    .filter_map(|import| import.as_name().map(Identifier::name))
                    .collect(),
                ..Names::default()
            },
        }))
    }
}

struct Usage<'a> {
    program: Program<'a>,
    ctx: &'a AnalyzeContext<'a>,
    names: Names<'a>,
}

#[derive(Default)]
struct Names<'a> {
    unprefixed: HashSet<&'a str>,
    prefixed: HashSet<(&'a str, &'a str)>,
    unknown: HashSet<&'a str>,
    prefixes: HashSet<&'a str>,
    extensions: HashSet<DeclarationIdentity>,
    unclassified: bool,
}

impl<'a> Names<'a> {
    fn record_segments(
        &mut self,
        model: &SemanticModel<'_>,
        segments: impl Iterator<Item = Identifier<'a>>,
        type_parameters: &TypeParameterScope,
    ) {
        // Constructors and constants may add a third segment after prefix.Type.
        // If a reference cannot be classified, keep the import, never drop it.
        if let Some(name) = QualifiedName::from_identifiers(segments.take(2)) {
            self.record(model, name, type_parameters);
        } else {
            self.unclassified = true;
        }
    }

    fn record(
        &mut self,
        model: &SemanticModel<'_>,
        name: QualifiedName<'a>,
        type_parameters: &TypeParameterScope,
    ) {
        if name.prefix.is_none() && type_parameters.lookup(name.name).is_some() {
            return;
        }
        if model
            .resolve_imported_member(name)
            .or_else(|| model.resolve_extension(name))
            .is_some()
        {
            match name.prefix {
                None => self.unprefixed.insert(name.name),
                Some(prefix) => self.prefixed.insert((prefix, name.name)),
            };
        } else if let Some(prefix) = name.prefix {
            // A qualified spelling can be a static member or named constructor,
            // not only an import prefix. Resolve its target independently.
            if self.prefixes.contains(prefix) {
                self.unknown.insert(prefix);
            } else {
                self.record(model, prefix.into(), type_parameters);
            }
        } else if !self.prefixes.contains(name.name)
            && model.resolve_name(name).is_none()
            && model.resolve_value(name).is_none()
            && model.resolve_sdk_member(name).is_none()
        {
            self.unknown.insert(name.name);
        }
    }
}

fn is_enclosing_member(node: Expr<'_>, state: &SemanticState<'_>) -> bool {
    state
        .referenced_declaration(node, false)
        .is_some_and(|reference| {
            reference.selector.kind != MemberSelectorKind::Declaration
                && !reference.selector.name.is_empty()
                && (state.constant_owner.as_ref() == Some(&reference.owner)
                    || state.enclosing_type.as_ref().is_some_and(|receiver| {
                        state
                            .signatures
                            .resolved_member_facts(receiver, reference.selector.name, &state.model)
                            .is_some()
                    }))
        })
}

fn operator_reference(node: Expr<'_>) -> Option<(Expr<'_>, &'static str, bool)> {
    match node {
        Expr::Binary(binary) => binary
            .op()
            .operator_name()
            .map(|name| (binary.left(), name, false)),
        Expr::Assign(assignment) => assignment
            .op()
            .operator_name()
            .map(|name| (assignment.target(), name, false)),
        Expr::Unary(unary) => match unary.op() {
            UnaryOp::Minus => Some((unary.operand(), "unary-", false)),
            UnaryOp::Tilde => Some((unary.operand(), "~", false)),
            UnaryOp::PlusPlus => Some((unary.operand(), "+", false)),
            UnaryOp::MinusMinus => Some((unary.operand(), "-", false)),
            UnaryOp::Bang => None,
        },
        Expr::PostfixIncDec(postfix) => Some((
            postfix.operand(),
            if postfix.op() == PostfixIncDec::Increment {
                "+"
            } else {
                "-"
            },
            false,
        )),
        Expr::Index(index) => Some((index.object(), "[]", index.is_null_safe())),
        _ => None,
    }
}

fn builtin_operator(receiver: &ResolvedType, name: &str) -> bool {
    // SDK operators are language-defined even without SDK source signatures.
    matches!(
        receiver.view(),
        ResolvedTypeView::Interface { identity: DeclarationIdentity::Sdk { library, name: ty }, nullable: false, .. }
            if library == "dart:core" && match ty.as_str() {
                "num" | "int" | "double" => matches!(name, "+" | "-" | "*" | "/" | "~/" | "%" | "<" | "<=" | ">" | ">=" | "==" | "unary-")
                    || (ty == "int" && matches!(name, "&" | "|" | "^" | "<<" | ">>" | ">>>" | "~")),
                "String" => matches!(name, "+" | "[]" | "=="),
                "bool" => matches!(name, "&" | "|" | "^" | "=="),
                _ => false,
            }
    )
}

impl Usage<'_> {
    fn record_owner(&mut self, owner: DeclarationIdentity, state: &SemanticState<'_>) {
        if state.signatures.extension_on_type(&owner).is_some() {
            self.names.extensions.insert(owner);
        }
    }
    fn record_field<'tree>(&mut self, access: FieldExpr<'tree>, state: &SemanticState<'tree>) {
        let qualifier = match access.object().unparenthesized() {
            Expr::Ident(identifier) => Some(identifier.name().into()),
            Expr::Field(field) => match field.object().unparenthesized() {
                Expr::Ident(prefix) => {
                    Some(QualifiedName::prefixed(prefix.name(), field.field().name()))
                }
                _ => None,
            },
            _ => None,
        };
        let namespace = matches!(
            access.object().unparenthesized(),
            Expr::Ident(prefix) if self.names.prefixes.contains(prefix.name())
        );
        let static_member = qualifier
            .and_then(|name| state.model.resolve_name(name))
            .is_some_and(|owner| {
                self.ctx.types().is_some_and(|types| {
                    matches!(
                        types.member_lookup_identity(&owner, access.field().name()),
                        MemberResult::Found(_)
                    )
                })
            });
        if !namespace && !static_member {
            let receiver = state.infer_access_receiver(
                access.object(),
                access.field().name(),
                access.is_null_safe(),
            );
            self.record_member(&receiver, access.field().name(), false, state);
        }
    }

    fn record_writes<'tree>(&mut self, node: Expr<'tree>, state: &SemanticState<'tree>) {
        let target = match node {
            Expr::Assign(assignment) => assignment.target(),
            Expr::PostfixIncDec(postfix) => postfix.operand(),
            Expr::Unary(unary) if matches!(unary.op(), UnaryOp::PlusPlus | UnaryOp::MinusMinus) => {
                unary.operand()
            }
            _ => return,
        };
        if let Some(reference) = state.referenced_declaration(target, true) {
            self.record_owner(reference.owner, state);
        }
        match target.unparenthesized() {
            Expr::Index(index) => {
                let receiver =
                    state.infer_access_receiver(index.object(), "[]=", index.is_null_safe());
                self.record_member(&receiver, "[]=", false, state);
            }
            Expr::Field(field) => {
                let receiver = state.infer_access_receiver(
                    field.object(),
                    field.field().name(),
                    field.is_null_safe(),
                );
                self.record_member(&receiver, field.field().name(), true, state);
            }
            _ => {}
        }
    }

    fn record_callable<'tree>(&mut self, node: Expr<'tree>, state: &SemanticState<'tree>) {
        if let Expr::Call(call) = node
            && !state.is_constructor_invocation(call.callee())
            && state.named_extension(call.callee()).is_none()
            && state.is_function_expression_invocation(call.callee())
        {
            let receiver = state.infer(call.callee());
            self.record_member(&receiver, "call", false, state);
        }
    }

    fn record_operator<'tree>(&mut self, node: Expr<'tree>, state: &SemanticState<'tree>) {
        if let Some((object, name, null_safe)) = operator_reference(node) {
            // Binary operands share the walker's expression-chain inference cache.
            let receiver = match node {
                Expr::Binary(_) => state.infer(object),
                Expr::Unary(unary)
                    if !matches!(unary.op(), UnaryOp::PlusPlus | UnaryOp::MinusMinus) =>
                {
                    state.infer(object)
                }
                _ => state.infer_access_receiver(object, name, null_safe),
            };
            if !builtin_operator(&receiver, name) {
                self.record_member(&receiver, name, false, state);
            }
        }
    }

    fn record_member(
        &mut self,
        receiver: &ResolvedType,
        name: &str,
        write: bool,
        state: &SemanticState<'_>,
    ) {
        if self.names.unclassified {
            return;
        }
        if let Some(owner) = state.referenced_member_owner(receiver, name, write) {
            self.record_owner(owner, state);
        } else if !matches!(receiver.view(), ResolvedTypeView::Dynamic)
            && !(name == "call"
                && (matches!(receiver.view(), ResolvedTypeView::Function { .. })
                    || receiver.interface("dart:core", "Function")))
        {
            if receiver.record_field(name).is_some() {
                return;
            }
            if let ResolvedTypeView::Interface { identity, .. } = receiver.view()
                && self.ctx.types().is_some_and(|types| {
                    matches!(
                        types.member_lookup_identity(identity, name),
                        MemberResult::Found(_)
                    )
                })
            {
                return;
            }
            self.names.unclassified = true;
        }
    }
}

impl<'a> SemanticRuleVisitor<'a> for Usage<'a> {
    fn finish(mut self: Box<Self>) -> Vec<Diagnostic> {
        if self.names.unclassified {
            return Vec::new();
        }
        let program = self.program;
        let ctx = self.ctx;

        let Some(identities) = ctx.identities() else {
            return Vec::new();
        };
        let model = falcon_analyze::SemanticModel::new(
            ctx.file_path(),
            ctx.parsed(),
            identities,
            ctx.types(),
        );
        let usage = &mut *self;
        let mut type_usage = TypeUsage {
            model: &model,
            names: &mut usage.names,
            type_parameters: TypeParameterScope::default(),
        };
        type_usage.visit_program(program);

        let imports: Vec<_> = program.imports().collect();
        let used: Vec<_> = imports
            .iter()
            .enumerate()
            .map(|(index, import)| used_names(index, *import, &self, &model))
            .collect();
        imports
            .iter()
            .enumerate()
            .filter(|(_, import)| import_targets_framework(**import))
            .filter(|(index, import)| {
                let Some(names) = &used[*index] else {
                    return false;
                };
                names.is_empty()
                    || imports.iter().enumerate().any(|(other, candidate)| {
                        candidate.as_name().map(Identifier::name)
                            == import.as_name().map(Identifier::name)
                            && used[other].as_ref().is_some_and(|other_names| {
                                names.len() < other_names.len() && names.is_subset(other_names)
                            })
                    })
            })
            .map(|(_, import)| {
                ctx.diagnostic("Remove this unnecessary Flutter import.", import.span())
            })
            .collect()
    }
    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'_>) {
        if self.names.unclassified {
            return;
        }
        self.record_callable(node, state);
        if self.names.unclassified {
            return;
        }
        if let Some(reference) = state
            .referenced_declaration(node, false)
            .or_else(|| state.referenced_declaration(node, true))
        {
            self.record_owner(reference.owner, state);
        } else if let Expr::Field(access) = node {
            self.record_field(access, state);
        }
        if self.names.unclassified {
            return;
        }
        self.record_operator(node, state);
        if self.names.unclassified {
            return;
        }
        self.record_writes(node, state);
        if self.names.unclassified {
            return;
        }
        match node {
            Expr::Ident(identifier)
                if !state.environment.is_bound(identifier.name())
                    && !is_enclosing_member(node, state) =>
            {
                self.names.record(
                    &state.model,
                    identifier.name().into(),
                    &state.type_parameters,
                );
                // An unresolved implicit-this reference may use a prefixed extension.
                if state.enclosing_type.is_some() && self.names.unknown.contains(identifier.name())
                {
                    self.names.unclassified = true;
                }
            }
            Expr::Field(access) => {
                let object = access.object();
                let field = access.field();
                if let Expr::Ident(prefix) = object.unparenthesized()
                    && !state.environment.is_bound(prefix.name())
                    && !is_enclosing_member(object.unparenthesized(), state)
                {
                    let names = QualifiedName::prefixed(prefix.name(), field.name());
                    self.names
                        .record(&state.model, names, &state.type_parameters);
                }
            }
            _ => {}
        }
    }

    fn visit_cascade_operation(
        &mut self,
        operation: CascadeOp<'a>,
        receiver: &ResolvedType,
        _section: Span,
        state: &SemanticState<'_>,
    ) {
        if self.names.unclassified {
            return;
        }
        let name = match operation {
            CascadeOp::Field(field) => field.name().name(),
            CascadeOp::Call(call) => call.name().name(),
            CascadeOp::Invoke(_) => "call",
            CascadeOp::Index(_) => "[]",
            CascadeOp::Assign(_) | CascadeOp::IndexAssign(_) | CascadeOp::NullAssert(_) => return,
        };
        self.record_member(receiver, name, false, state);
    }

    fn visit_cascade_assignment(
        &mut self,
        assignment: CascadeAssignment<'a>,
        receiver: &ResolvedType,
        _section: Span,
        state: &SemanticState<'_>,
    ) {
        if self.names.unclassified {
            return;
        }
        // The callback supplies the receiver, not the intermediate operator receiver.
        if assignment.operator.operator_name().is_some() {
            self.names.unclassified = true;
            return;
        }
        if assignment.indexed {
            self.record_member(receiver, "[]=", false, state);
            if assignment.operator != AssignOp::Eq {
                self.record_member(receiver, "[]", false, state);
            }
        } else if let Expr::Ident(name) = assignment.target.unparenthesized() {
            self.record_member(receiver, name.name(), true, state);
            if assignment.operator != AssignOp::Eq {
                self.record_member(receiver, name.name(), false, state);
            }
        } else {
            self.names.unclassified = true;
        }
    }
    fn visit_pattern(
        &mut self,
        node: Pattern<'a>,
        _matched: Option<&ResolvedType>,
        state: &SemanticState<'_>,
    ) {
        if self.names.unclassified {
            return;
        }
        if let Pattern::Object(pattern) = node {
            let receiver = state
                .model
                .resolve_type_in(pattern.type_(), &state.type_parameters);
            for field in pattern.fields() {
                self.record_member(&receiver, field.name().name(), false, state);
                if self.names.unclassified {
                    break;
                }
            }
        }
    }
}

struct TypeUsage<'a, 'model, 'tree> {
    model: &'a falcon_analyze::SemanticModel<'model>,
    names: &'a mut Names<'tree>,
    type_parameters: TypeParameterScope,
}

impl<'tree> TypeUsage<'_, '_, 'tree> {
    fn scoped(
        &mut self,
        parameters: impl Iterator<Item = TypeParam<'tree>> + Clone,
        walk: impl FnOnce(&mut Self),
    ) {
        self.type_parameters.push(parameters, self.model);
        walk(self);
        self.type_parameters.pop();
    }
}

impl<'tree> Visitor<'tree> for TypeUsage<'_, '_, 'tree> {
    fn visit_class_decl(&mut self, node: ClassDecl<'tree>) {
        self.scoped(node.type_params(), |v| walk_class_decl(v, node));
    }
    fn visit_class_type_alias(&mut self, node: ClassTypeAliasDecl<'tree>) {
        self.scoped(node.type_params(), |v| walk_class_type_alias(v, node));
    }
    fn visit_mixin_decl(&mut self, node: MixinDecl<'tree>) {
        self.scoped(node.type_params(), |v| walk_mixin_decl(v, node));
    }
    fn visit_mixin_class(&mut self, node: MixinClassDecl<'tree>) {
        self.scoped(node.type_params(), |v| walk_mixin_class(v, node));
    }
    fn visit_enum_decl(&mut self, node: EnumDecl<'tree>) {
        self.scoped(node.type_params(), |v| walk_enum_decl(v, node));
    }
    fn visit_extension_decl(&mut self, node: ExtensionDecl<'tree>) {
        self.scoped(node.type_params(), |v| walk_extension_decl(v, node));
    }
    fn visit_extension_type(&mut self, node: ExtensionTypeDecl<'tree>) {
        self.scoped(node.type_params(), |v| walk_extension_type(v, node));
    }
    fn visit_function_decl(&mut self, node: FunctionDecl<'tree>) {
        self.scoped(node.type_params(), |v| walk_function_decl(v, node));
    }
    fn visit_method_decl(&mut self, node: MethodDecl<'tree>) {
        self.scoped(node.type_params(), |v| walk_method_decl(v, node));
    }
    fn visit_type_alias_decl(&mut self, node: TypeAliasDecl<'tree>) {
        self.scoped(node.type_params(), |v| walk_type_alias_decl(v, node));
    }
    fn visit_function_type(&mut self, node: FunctionType<'tree>) {
        self.scoped(node.type_params(), |v| walk_function_type(v, node));
    }
    fn visit_formal_param(&mut self, node: FormalParam<'tree>) {
        self.scoped(node.type_params(), |v| walk_formal_param(v, node));
    }
    fn visit_local_func_decl(&mut self, node: LocalFuncDecl<'tree>) {
        self.scoped(node.type_params(), |v| walk_local_func_decl(v, node));
    }
    fn visit_expr(&mut self, node: Expr<'tree>) {
        walk_expr_chain(self, node, |v, expr| {
            if matches!(expr, Expr::Error(_)) {
                v.names.unclassified = true;
            }
            if let Expr::FuncExpr(function) = expr {
                v.scoped(function.type_params(), |v| walk_expr(v, expr));
                false
            } else {
                true
            }
        });
    }
    fn visit_dart_type(&mut self, node: DartType<'tree>) {
        match node {
            DartType::Named(named) => {
                self.names
                    .record_segments(self.model, named.segments(), &self.type_parameters);
            }
            // These types have no direct name; their component types are walked.
            DartType::Function(_)
            | DartType::Record(_)
            | DartType::Void(_)
            | DartType::Dynamic(_) => {}
        }
        walk_dart_type(self, node);
    }

    fn visit_annotation(&mut self, node: Annotation<'tree>) {
        self.names
            .record_segments(self.model, node.name(), &self.type_parameters);
        walk_annotation(self, node);
    }

    fn visit_const_pattern_value(&mut self, node: ConstPatternValue<'tree>) {
        match node {
            ConstPatternValue::Name(names) => {
                self.names
                    .record_segments(self.model, names, &self.type_parameters);
            }
            ConstPatternValue::Expr(expr) => {
                walk_const_pattern_value(self, ConstPatternValue::Expr(expr));
            }
        }
    }

    fn visit_pattern(&mut self, node: Pattern<'tree>) {
        if matches!(node, Pattern::Error(_)) {
            self.names.unclassified = true;
        }
        walk_pattern(self, node);
    }
}

fn import_targets_framework(import: ImportDirective<'_>) -> bool {
    std::iter::once(import.uri())
        .chain(import.configurable_uris().map(ConfigurableUri::uri))
        .any(|uri| uri.value().starts_with("package:flutter/") || uri.value() == "dart:async")
}

fn used_names<'a>(
    index: usize,
    import: ImportDirective<'a>,
    usage: &'a Usage<'_>,
    model: &SemanticModel<'_>,
) -> Option<HashSet<&'a str>> {
    let prefix = import.as_name().map(Identifier::name);
    if usage.names.unclassified
        || prefix.is_some_and(|prefix| usage.names.unknown.contains(prefix))
        || (prefix.is_none() && usage.names.unknown.iter().any(|name| allows(import, name)))
    {
        return None;
    }
    let usage = &usage.names;
    let mut names = HashSet::with_capacity(usage.unprefixed.len() + usage.prefixed.len());
    let used = usage
        .unprefixed
        .iter()
        .copied()
        .filter(|_| prefix.is_none())
        .chain(
            usage
                .prefixed
                .iter()
                .filter_map(|(used_prefix, name)| (Some(*used_prefix) == prefix).then_some(*name)),
        );
    for name in used {
        let qualified = QualifiedName { prefix, name };
        let identity = model
            .resolve_imported_member(qualified)
            .or_else(|| model.resolve_extension(qualified))?;
        if model.import_provides(index, qualified, &identity)? {
            names.insert(name);
        }
    }
    for extension in &usage.extensions {
        let name = QualifiedName {
            prefix,
            name: extension.name(),
        };
        if model.import_provides(index, name, extension)? {
            names.insert(extension.name());
        }
    }
    Some(names)
}

fn allows(import: ImportDirective<'_>, name: &str) -> bool {
    import.combinators().all(|combinator| match combinator {
        ImportCombinator::Show(names) => names.names().any(|shown| shown.name() == name),
        ImportCombinator::Hide(names) => names.names().all(|hidden| hidden.name() != name),
    })
}
