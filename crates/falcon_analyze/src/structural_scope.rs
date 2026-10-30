//! Shared source-order collectors, driven exclusively by the syntax Visitor walkers.
//!
//! Collectors own their context and diagnostics. Every delivered enter has a matching
//! exit, including `SkipChildren`; a rejected child has neither. Pruning and routing
//! affect only that collector, never the physical traversal or another collector.

use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::visit::{self as visitor, Visitor};
use falcon_syntax::cst::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StructuralTraversal {
    Continue,
    SkipChildren,
}

macro_rules! structural_nodes {
    (@visit visit_collection_element, $ty:ty) => {};
    (@visit visit_map_element, $ty:ty) => {};
    (@method visit_expr, $kind:ident, $ty:ty, [$($walk:ident)?]) => {
        fn visit_expr(&mut self, node: $ty) {
            visitor::walk_expr_chain_with_exit(
                self,
                node,
                |visitor, node| {
                    let child = StructuralNode::Expr(node);
                    visitor.enter_node(child.clone());
                    visitor.expression_parents.push(visitor.parent.replace(child));
                    visitor.depth += 1;
                    true
                },
                |visitor, node| {
                    visitor.depth -= 1;
                    visitor.parent = visitor.expression_parents.pop().unwrap();
                    visitor.exit_node(StructuralNode::Expr(node));
                },
            );
        }
    };
    (@method $visit:ident, $kind:ident, $ty:ty, [$($walk:ident)?]) => {
        fn $visit(&mut self, node: $ty) {
            let child = StructuralNode::$kind(node.clone());
            self.enter_node(child.clone());
            $(
                let mut children = StructuralVisitors {
                    collectors: self.collectors,
                    subscribers: self.subscribers,
                    parent: Some(child.clone()),
                    depth: self.depth + 1,
                    expression_parents: self.expression_parents,
                };
                structural_nodes!(@walk $walk, &mut children, node.clone());
            )?
            self.exit_node(child);
        }
    };
    (@visit $visit:ident, $ty:ty) => {
        fn $visit(&mut self, node: $ty) {
            StructuralVisitors::$visit(self, node);
        }
    };
    (@walk walk_element, $visitor:expr, $node:expr) => {
        visitor::walk_element($visitor, $node.kind());
    };
    (@walk $walk:ident, $visitor:expr, $node:expr) => {
        visitor::$walk($visitor, $node);
    };
    ($lifetime:lifetime; $(
        $kind:ident, $visit:ident, $exit:ident, $ty:ty, [$($walk:ident)?];
    )*) => {
        /// A node delivered by the existing structural Visitor contract.
        #[derive(Clone)]
        pub enum StructuralNode<$lifetime> {
            $($kind($ty),)*
        }

        /// Node kinds used to subscribe to individual structural hooks.
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        #[repr(usize)]
        pub enum StructuralKind {
            $($kind,)*
        }

        impl StructuralKind {
            const ALL: &'static [Self] = &[$(Self::$kind,)*];
        }

        const _: () = assert!(StructuralKind::ALL.len() <= u128::BITS as usize);

        /// An independent collector; callbacks must not recurse into the tree.
        ///
        /// Use [`crate::structural_visitor!`] to derive subscriptions from the hooks.
        /// A direct implementation defaults to all hooks and remains correct.
        pub trait StructuralRuleVisitor<$lifetime> {
            /// Consume this collector once, after every matching exit hook.
            fn finish(self: Box<Self>) -> Vec<Diagnostic>;

            fn interests(&self) -> StructuralInterests {
                StructuralInterests::all()
            }

            /// Whether subscriptions were generated from the implementation's hooks.
            fn has_derived_interests(&self) -> bool {
                false
            }
            fn route_child(
                &mut self,
                _parent: StructuralNode<$lifetime>,
                _child: StructuralNode<$lifetime>,
            ) -> bool {
                true
            }

            $(
                fn $visit(&mut self, _node: $ty) -> StructuralTraversal {
                    StructuralTraversal::Continue
                }

                fn $exit(&mut self, _node: $ty) {}
            )*
        }

        impl StructuralInterests {
            /// Macro support; derive subscriptions from the implemented method names.
            #[doc(hidden)]
            #[must_use]
            pub const fn with_hook(self, name: &str) -> Self {
                $(
                    if Self::hook_name_eq(name, stringify!($visit)) {
                        return self.enter(StructuralKind::$kind);
                    }
                    if Self::hook_name_eq(name, stringify!($exit)) {
                        return self.exit(StructuralKind::$kind);
                    }
                )*
                if Self::hook_name_eq(name, "route_child") {
                    self.routing()
                } else if Self::hook_name_eq(name, "finish") {
                    self
                } else {
                    panic!("not a structural collector hook")
                }
            }
        }

        #[cfg(test)]
        #[test]
        fn structural_hook_schema_matches_syntax_visitor() {
            use std::collections::BTreeSet;

            let source = include_str!("../../falcon_syntax/src/cst/visit.rs");
            let trait_body = source.split_once("pub trait Visitor<'a>: Sized {").unwrap().1;
            let trait_body = trait_body.split_once("\n}").unwrap().0;
            let syntax_hooks: BTreeSet<_> = trait_body.lines().filter_map(|line| {
                line.trim_start().strip_prefix("fn ")
                    .map(|signature| signature.split(['(', '<']).next().unwrap())
            }).collect();
            let mut structural_hooks: BTreeSet<_> = [$(stringify!($visit),)*].into_iter().collect();
            structural_hooks.remove("visit_collection_element");
            structural_hooks.remove("visit_map_element");
            structural_hooks.insert("visit_element");
            assert_eq!(
                structural_hooks, syntax_hooks,
                "every syntax Visitor hook must have structural enter/exit, node and dispatch entries"
            );
        }

        impl<$lifetime> Visitor<$lifetime> for StructuralVisitors<'_, $lifetime> {
            $(structural_nodes!(@visit $visit, $ty);)*

            fn visit_element<E: Element<$lifetime>>(&mut self, element: E) {
                match element.view() {
                    ElementView::Map(element) => self.visit_map_element(element),
                    ElementView::Collection(element) => self.visit_collection_element(element),
                }
            }
        }

        impl<$lifetime> StructuralVisitors<'_, $lifetime> {
            $(structural_nodes!(@method $visit, $kind, $ty, [$($walk)?]);)*

            fn enter_node(&mut self, child: StructuralNode<$lifetime>) {
                match child.clone() {
                    $(StructuralNode::$kind(node) => {
                        let kind = StructuralKind::$kind;
                        for &index in self.subscribers.for_kind(kind) {
                            let collector = &mut self.collectors[index];
                            if collector.suppressed.is_some() {
                                continue;
                            }
                            if collector.interests.routing
                                && let Some(parent) = self.parent.clone()
                                && !collector.visitor.route_child(parent, child.clone())
                            {
                                collector.suppressed = Some((self.depth, false));
                                continue;
                            }
                            if collector.interests.enters(kind)
                                && collector.visitor.$visit(node.clone()) == StructuralTraversal::SkipChildren
                            {
                                collector.suppressed = Some((self.depth, true));
                            }
                        }
                    })*
                }
            }

            fn exit_node(&mut self, child: StructuralNode<$lifetime>) {
                match child {
                    $(StructuralNode::$kind(node) => {
                        let kind = StructuralKind::$kind;
                        for &index in self.subscribers.for_kind(kind) {
                            let collector = &mut self.collectors[index];
                            if let Some((owner, entered)) = collector.suppressed {
                                if owner != self.depth {
                                    continue;
                                }
                                collector.suppressed = None;
                                if !entered {
                                    continue;
                                }
                            }
                            if collector.interests.exits(kind) {
                                collector.visitor.$exit(node.clone());
                            }
                        }
                    })*
                }
            }
        }
    };
}

structural_nodes! {
    'n;
    Program, visit_program, exit_program, Program<'n>, [walk_program];
    TypeAliasDecl, visit_type_alias_decl, exit_type_alias_decl, TypeAliasDecl<'n>, [walk_type_alias_decl];
    Import, visit_import, exit_import, ImportDirective<'n>, [walk_import];
    Export, visit_export, exit_export, ExportDirective<'n>, [walk_export];
    TopLevelDecl, visit_top_level_decl, exit_top_level_decl, TopLevelDecl<'n>, [walk_top_level_decl];
    ClassDecl, visit_class_decl, exit_class_decl, ClassDecl<'n>, [walk_class_decl];
    MixinDecl, visit_mixin_decl, exit_mixin_decl, MixinDecl<'n>, [walk_mixin_decl];
    EnumDecl, visit_enum_decl, exit_enum_decl, EnumDecl<'n>, [walk_enum_decl];
    ExtensionDecl, visit_extension_decl, exit_extension_decl, ExtensionDecl<'n>, [walk_extension_decl];
    FunctionDecl, visit_function_decl, exit_function_decl, FunctionDecl<'n>, [walk_function_decl];
    ClassMember, visit_class_member, exit_class_member, ClassMember<'n>, [walk_class_member];
    FieldDecl, visit_field_decl, exit_field_decl, FieldDecl<'n>, [walk_field_decl];
    Constructor, visit_constructor, exit_constructor, ConstructorRef<'n>, [];
    ConstructorDecl, visit_constructor_decl, exit_constructor_decl, ConstructorDecl<'n>, [walk_constructor_decl];
    MethodDecl, visit_method_decl, exit_method_decl, MethodDecl<'n>, [walk_method_decl];
    GetterDecl, visit_getter_decl, exit_getter_decl, GetterDecl<'n>, [walk_getter_decl];
    SetterDecl, visit_setter_decl, exit_setter_decl, SetterDecl<'n>, [walk_setter_decl];
    DartType, visit_dart_type, exit_dart_type, DartType<'n>, [walk_dart_type];
    Stmt, visit_stmt, exit_stmt, Stmt<'n>, [walk_stmt];
    Block, visit_block, exit_block, Block<'n>, [walk_block];
    FunctionBody, visit_function_body, exit_function_body, FunctionBody<'n>, [walk_function_body];
    SwitchCase, visit_switch_case, exit_switch_case, SwitchCase<'n>, [walk_switch_case];
    CatchClause, visit_catch_clause, exit_catch_clause, CatchClause<'n>, [walk_catch_clause];
    LocalVar, visit_local_var, exit_local_var, LocalVarDecl<'n>, [walk_local_var];
    ForHeader, visit_for_header, exit_for_header, ForHeader<'n>, [walk_for_header];
    Expr, visit_expr, exit_expr, Expr<'n>, [walk_expr];
    CollectionElement, visit_collection_element, exit_collection_element, CollectionElement<'n>, [walk_element];
    MapElement, visit_map_element, exit_map_element, MapElement<'n>, [walk_element];
    SwitchExprArm, visit_switch_expr_arm, exit_switch_expr_arm, SwitchExprArm<'n>, [walk_switch_expr_arm];
    Pattern, visit_pattern, exit_pattern, Pattern<'n>, [walk_pattern];
    Annotation, visit_annotation, exit_annotation, Annotation<'n>, [walk_annotation];
    Identifier, visit_identifier, exit_identifier, Identifier<'n>, [];
    StringLit, visit_string_lit, exit_string_lit, StringLitNode<'n>, [];
    FormalParam, visit_formal_param, exit_formal_param, FormalParam<'n>, [walk_formal_param];
    PrimaryConstructor, visit_primary_constructor, exit_primary_constructor, PrimaryConstructor<'n>, [walk_primary_constructor];
    PrimaryConstructorBody, visit_primary_constructor_body, exit_primary_constructor_body, PrimaryConstructorBody<'n>, [walk_primary_constructor_body];
    LibraryDirective, visit_library_directive, exit_library_directive, LibraryDirective<'n>, [walk_library_directive];
    PartOfDirective, visit_part_of_directive, exit_part_of_directive, PartOfDirective<'n>, [walk_part_of_directive];
    PartDirective, visit_part_directive, exit_part_directive, PartDirective<'n>, [walk_part_directive];
    ConfigurableUri, visit_configurable_uri, exit_configurable_uri, ConfigurableUri<'n>, [walk_configurable_uri];
    ImportCombinator, visit_import_combinator, exit_import_combinator, ImportCombinator<'n>, [walk_import_combinator];
    ClassTypeAlias, visit_class_type_alias, exit_class_type_alias, ClassTypeAliasDecl<'n>, [walk_class_type_alias];
    MixinClass, visit_mixin_class, exit_mixin_class, MixinClassDecl<'n>, [walk_mixin_class];
    ExtensionType, visit_extension_type, exit_extension_type, ExtensionTypeDecl<'n>, [walk_extension_type];
    TopLevelVar, visit_top_level_var, exit_top_level_var, TopLevelVarDecl<'n>, [walk_top_level_var];
    OperatorDecl, visit_operator_decl, exit_operator_decl, OperatorDecl<'n>, [walk_operator_decl];
    ConstructorInitializer, visit_constructor_initializer, exit_constructor_initializer, ConstructorInitializer<'n>, [walk_constructor_initializer];
    ArgList, visit_arg_list, exit_arg_list, ArgList<'n>, [walk_arg_list];
    CascadeSection, visit_cascade_section, exit_cascade_section, CascadeSection<'n>, [walk_cascade_section];
    ForInit, visit_for_init, exit_for_init, ForInit<'n>, [walk_for_init];
    EnumVariant, visit_enum_variant, exit_enum_variant, EnumVariant<'n>, [walk_enum_variant];
    VarDeclarator, visit_var_declarator, exit_var_declarator, VarDeclarator<'n>, [walk_var_declarator];
    RedirectedConstructor, visit_redirected_constructor, exit_redirected_constructor, RedirectedConstructor<'n>, [walk_redirected_constructor];
    TypeParam, visit_type_param, exit_type_param, TypeParam<'n>, [walk_type_param];
    FormalParamList, visit_formal_param_list, exit_formal_param_list, FormalParamList<'n>, [walk_formal_param_list];
    NamedType, visit_named_type, exit_named_type, NamedType<'n>, [walk_named_type];
    FunctionType, visit_function_type, exit_function_type, FunctionType<'n>, [walk_function_type];
    FunctionTypeParam, visit_function_type_param, exit_function_type_param, FunctionTypeParam<'n>, [walk_function_type_param];
    RecordType, visit_record_type, exit_record_type, RecordType<'n>, [walk_record_type];
    NamedRecordField, visit_named_record_field, exit_named_record_field, NamedRecordField<'n>, [walk_named_record_field];
    IfCondition, visit_if_condition, exit_if_condition, IfCondition<'n>, [walk_if_condition];
    IfStmt, visit_if_stmt, exit_if_stmt, IfStmt<'n>, [walk_if_stmt];
    ForStmt, visit_for_stmt, exit_for_stmt, ForStmt<'n>, [walk_for_stmt];
    WhileStmt, visit_while_stmt, exit_while_stmt, WhileStmt<'n>, [walk_while_stmt];
    DoWhileStmt, visit_do_while_stmt, exit_do_while_stmt, DoWhileStmt<'n>, [walk_do_while_stmt];
    SwitchStmt, visit_switch_stmt, exit_switch_stmt, SwitchStmt<'n>, [walk_switch_stmt];
    TryCatchStmt, visit_try_catch_stmt, exit_try_catch_stmt, TryCatchStmt<'n>, [walk_try_catch_stmt];
    ReturnStmt, visit_return_stmt, exit_return_stmt, ReturnStmt<'n>, [walk_return_stmt];
    ThrowStmt, visit_throw_stmt, exit_throw_stmt, ThrowStmt<'n>, [walk_throw_stmt];
    BreakStmt, visit_break_stmt, exit_break_stmt, BreakStmt<'n>, [walk_break_stmt];
    ContinueStmt, visit_continue_stmt, exit_continue_stmt, ContinueStmt<'n>, [walk_continue_stmt];
    PatternDeclaration, visit_pattern_declaration, exit_pattern_declaration, PatternDeclaration<'n>, [walk_pattern_declaration];
    PatternAssignStmt, visit_pattern_assign_stmt, exit_pattern_assign_stmt, PatternAssignStmt<'n>, [walk_pattern_assign_stmt];
    LabeledStmt, visit_labeled_stmt, exit_labeled_stmt, LabeledStmt<'n>, [walk_labeled_stmt];
    LocalFuncDecl, visit_local_func_decl, exit_local_func_decl, LocalFuncDecl<'n>, [walk_local_func_decl];
    AssertStmt, visit_assert_stmt, exit_assert_stmt, AssertStmt<'n>, [walk_assert_stmt];
    YieldStmt, visit_yield_stmt, exit_yield_stmt, YieldStmt<'n>, [walk_yield_stmt];
    ExprStmt, visit_expr_stmt, exit_expr_stmt, ExprStmt<'n>, [walk_expr_stmt];
    ErrorNode, visit_error_node, exit_error_node, ErrorNode<'n>, [];
    SwitchCaseKind, visit_switch_case_kind, exit_switch_case_kind, SwitchCaseKind<'n>, [walk_switch_case_kind];
    CascadeOp, visit_cascade_op, exit_cascade_op, CascadeOp<'n>, [walk_cascade_op];
    MapEntry, visit_map_entry, exit_map_entry, MapEntry<'n>, [walk_map_entry];
    RecordField, visit_record_field, exit_record_field, RecordField<'n>, [walk_record_field];
    NamedArg, visit_named_arg, exit_named_arg, NamedArg<'n>, [walk_named_arg];
    StringInterpolation, visit_string_interpolation, exit_string_interpolation, StringInterpolation<'n>, [walk_string_interpolation];
    LiteralPattern, visit_literal_pattern, exit_literal_pattern, LiteralPattern<'n>, [walk_literal_pattern];
    LiteralPatternValue, visit_literal_pattern_value, exit_literal_pattern_value, LiteralPatternValue<'n>, [walk_literal_pattern_value];
    ConstPattern, visit_const_pattern, exit_const_pattern, ConstPattern<'n>, [walk_const_pattern];
    ConstPatternValue, visit_const_pattern_value, exit_const_pattern_value, ConstPatternValue<'n>, [walk_const_pattern_value];
    ListPattern, visit_list_pattern, exit_list_pattern, ListPattern<'n>, [walk_list_pattern];
    ListPatternElement, visit_list_pattern_element, exit_list_pattern_element, ListPatternElement<'n>, [walk_list_pattern_element];
    RecordPattern, visit_record_pattern, exit_record_pattern, RecordPattern<'n>, [walk_record_pattern];
    RecordPatternField, visit_record_pattern_field, exit_record_pattern_field, RecordPatternField<'n>, [walk_record_pattern_field];
    MapPattern, visit_map_pattern, exit_map_pattern, MapPattern<'n>, [walk_map_pattern];
    MapPatternEntry, visit_map_pattern_entry, exit_map_pattern_entry, MapPatternEntry<'n>, [walk_map_pattern_entry];
    ObjectPattern, visit_object_pattern, exit_object_pattern, ObjectPattern<'n>, [walk_object_pattern];
    ObjectPatternField, visit_object_pattern_field, exit_object_pattern_field, ObjectPatternField<'n>, [walk_object_pattern_field];
    FunctionKind, visit_function_kind, exit_function_kind, FunctionKind<'n>, [walk_function_kind];
    Name, visit_name, exit_name, Identifier<'n>, [];
    NativeString, visit_native_string, exit_native_string, StringLitNode<'n>, [];
}

/// Implement a structural collector and derive subscriptions from its hooks.
///
/// Wrap the complete implementation, including `finish` and `route_child`.
/// Do not declare `interests` or `has_derived_interests` manually.
///
/// ```
/// use falcon_analyze::{StructuralRuleVisitor, StructuralTraversal};
/// use falcon_diagnostics::Diagnostic;
/// use falcon_syntax::cst::Expr;
///
/// struct Collector {
///     diags: Vec<Diagnostic>,
/// }
///
/// falcon_analyze::structural_visitor! {
///     impl<'a> StructuralRuleVisitor<'a> for Collector {
///         fn visit_expr(&mut self, _node: Expr<'a>) -> StructuralTraversal {
///             StructuralTraversal::Continue
///         }
///
///         fn finish(self: Box<Self>) -> Vec<Diagnostic> {
///             self.diags
///         }
///     }
/// }
/// ```
#[macro_export]
macro_rules! structural_visitor {
    (
        impl $(<$($lifetime:lifetime),*>)? $visitor:ident <$tree:lifetime> for $collector:ty {
            $(
                $(#[$attribute:meta])*
                fn $method:ident $args:tt $(-> $result:ty)? $body:block
            )*
        }
    ) => {
        impl $(<$($lifetime),*>)? $visitor<$tree> for $collector {
            fn interests(&self) -> $crate::StructuralInterests {
                const INTERESTS: $crate::StructuralInterests = $crate::StructuralInterests::new()
                    $(.with_hook(stringify!($method)))*;
                INTERESTS
            }

            fn has_derived_interests(&self) -> bool {
                true
            }

            $(
                $(#[$attribute])*
                fn $method $args $(-> $result)? $body
            )*
        }
    };
}

/// Static subscriptions, computed once per collector, not once per node.
///
/// Prefer [`crate::structural_visitor!`] over manually constructing subscriptions.
/// Direct trait implementations default to all hooks, so they cannot silently lose
/// callbacks by omitting this optimization.
#[derive(Clone, Copy)]
pub struct StructuralInterests {
    enter: u128,
    exit: u128,
    routing: bool,
}

impl Default for StructuralInterests {
    /// A default subscription must never suppress a directly implemented hook.
    fn default() -> Self {
        Self::all()
    }
}

impl StructuralInterests {
    const fn hook_name_eq(left: &str, right: &str) -> bool {
        let left = left.as_bytes();
        let right = right.as_bytes();
        if left.len() != right.len() {
            return false;
        }
        let mut index = 0;
        while index < left.len() {
            if left[index] != right[index] {
                return false;
            }
            index += 1;
        }
        true
    }

    #[must_use]
    pub const fn new() -> Self {
        Self {
            enter: 0,
            exit: 0,
            routing: false,
        }
    }

    #[must_use]
    pub const fn all() -> Self {
        Self {
            enter: u128::MAX,
            exit: u128::MAX,
            routing: true,
        }
    }

    #[must_use]
    pub const fn enter(mut self, kind: StructuralKind) -> Self {
        self.enter |= 1 << kind as usize;
        self
    }

    #[must_use]
    pub const fn exit(mut self, kind: StructuralKind) -> Self {
        self.exit |= 1 << kind as usize;
        self
    }

    #[must_use]
    pub const fn routing(mut self) -> Self {
        self.routing = true;
        self
    }

    fn enters(self, kind: StructuralKind) -> bool {
        self.enter & (1 << kind as usize) != 0
    }

    fn exits(self, kind: StructuralKind) -> bool {
        self.exit & (1 << kind as usize) != 0
    }
}

struct Collector<'a> {
    visitor: Box<dyn StructuralRuleVisitor<'a> + 'a>,
    interests: StructuralInterests,
    // Owner depth and whether its enter was delivered. Only that owner's exit
    // clears suppression, so every descendant and sibling child remains hidden.
    suppressed: Option<(usize, bool)>,
}

struct Subscriptions {
    offsets: [usize; StructuralKind::ALL.len() + 1],
    collectors: Vec<usize>,
}

impl Subscriptions {
    fn new(collectors: &[Collector<'_>]) -> Self {
        let kind_count = StructuralKind::ALL.len();
        let kind_mask = u128::MAX >> (u128::BITS as usize - kind_count);
        let capacity = collectors
            .iter()
            .map(|collector| {
                let interests = collector.interests;
                if interests.routing {
                    kind_count
                } else {
                    ((interests.enter | interests.exit) & kind_mask).count_ones() as usize
                }
            })
            .sum();
        let mut subscriptions = Self {
            offsets: [0; StructuralKind::ALL.len() + 1],
            collectors: Vec::with_capacity(capacity),
        };
        for &kind in StructuralKind::ALL {
            for (index, collector) in collectors.iter().enumerate() {
                let interests = collector.interests;
                if interests.routing || interests.enters(kind) || interests.exits(kind) {
                    subscriptions.collectors.push(index);
                }
            }
            subscriptions.offsets[kind as usize + 1] = subscriptions.collectors.len();
        }
        subscriptions
    }

    fn for_kind(&self, kind: StructuralKind) -> &[usize] {
        &self.collectors[self.offsets[kind as usize]..self.offsets[kind as usize + 1]]
    }
}

struct StructuralVisitors<'v, 'a> {
    collectors: &'v mut [Collector<'a>],
    subscribers: &'v Subscriptions,
    parent: Option<StructuralNode<'a>>,
    depth: usize,
    expression_parents: &'v mut Vec<Option<StructuralNode<'a>>>,
}

/// Run one existing Visitor traversal and consume every collector in input order.
/// Subscription tables and collector state allocate only once per program.
pub(crate) fn visit_program<'a>(
    program: Program<'a>,
    visitors: Vec<Box<dyn StructuralRuleVisitor<'a> + 'a>>,
) -> Vec<Vec<Diagnostic>> {
    if visitors.is_empty() {
        return Vec::new();
    }
    let mut collectors: Vec<_> = visitors
        .into_iter()
        .map(|visitor| {
            let interests = visitor.interests();
            Collector {
                visitor,
                interests,
                suppressed: None,
            }
        })
        .collect();
    let subscribers = Subscriptions::new(&collectors);
    StructuralVisitors {
        collectors: &mut collectors,
        subscribers: &subscribers,
        parent: None,
        depth: 0,
        expression_parents: &mut Vec::new(),
    }
    .visit_program(program);
    collectors
        .into_iter()
        .map(|collector| collector.visitor.finish())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    struct BalancedExpressions<'a> {
        stack: Vec<Expr<'a>>,
        count: usize,
        expected: usize,
        skip: bool,
        reject_right: bool,
    }

    crate::structural_visitor! {
        impl<'a> StructuralRuleVisitor<'a> for BalancedExpressions<'a> {
            fn route_child(&mut self, parent: StructuralNode<'a>, child: StructuralNode<'a>) -> bool {
                if let StructuralNode::Expr(parent) = parent {
                    assert_eq!(self.stack.last().unwrap().syntax().id(), parent.syntax().id());
                }
                !(self.reject_right
                    && matches!(child, StructuralNode::Expr(Expr::Ident(name)) if name.name() == "right"))
            }

            fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
                self.stack.push(node);
                self.count += 1;
                if self.skip {
                    StructuralTraversal::SkipChildren
                } else {
                    StructuralTraversal::Continue
                }
            }

            fn exit_expr(&mut self, node: Expr<'a>) {
                assert_eq!(self.stack.pop().unwrap().syntax().id(), node.syntax().id());
            }

            fn finish(self: Box<Self>) -> Vec<Diagnostic> {
                assert!(self.stack.is_empty());
                assert_eq!(self.count, self.expected);
                Vec::new()
            }
        }
    }

    #[test]
    fn expression_dispatch_preserves_nested_hooks_routing_and_independent_pruning() {
        let source = "var value = left + call(right);";
        let parsed = falcon_dart_parser::parse(source);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let visitors = [(false, false, 5), (true, false, 1), (false, true, 4)]
            .into_iter()
            .map(|(skip, reject_right, expected)| {
                Box::new(BalancedExpressions {
                    stack: Vec::new(),
                    count: 0,
                    expected,
                    skip,
                    reject_right,
                }) as Box<dyn StructuralRuleVisitor<'_>>
            })
            .collect();
        visit_program(parsed.tree.program(source), visitors);
    }

    #[test]
    fn expression_dispatch_handles_hundred_thousand_operands_on_normal_stack() {
        const OPERANDS: usize = 100_000;
        for (operand, operator) in [
            ("1", "+"),
            ("true", "&&"),
            ("false", "||"),
            ("null", "??"),
            ("'s'", "+"),
        ] {
            let mut source = String::from("var value = ");
            source.push_str(operand);
            for _ in 1..OPERANDS {
                source.push_str(operator);
                source.push_str(operand);
            }
            source.push(';');
            let parsed = falcon_dart_parser::parse(&source);
            assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
            visit_program(
                parsed.tree.program(&source),
                vec![Box::new(BalancedExpressions {
                    stack: Vec::new(),
                    count: 0,
                    expected: OPERANDS * 2 - 1,
                    skip: false,
                    reject_right: false,
                })],
            );
        }
    }

    struct Subscribed;

    crate::structural_visitor! {
        impl<'a> StructuralRuleVisitor<'a> for Subscribed {
            fn visit_expr(&mut self, _node: Expr<'a>) -> StructuralTraversal {
                StructuralTraversal::Continue
            }

            fn exit_program(&mut self, _node: Program<'a>) {}

            fn route_child(&mut self, _parent: StructuralNode<'_>, _child: StructuralNode<'_>) -> bool {
                true
            }

            fn finish(self: Box<Self>) -> Vec<Diagnostic> {
                Vec::new()
            }
        }
    }

    struct RetainedExpressions<'a> {
        expressions: Vec<Expr<'a>>,
    }

    crate::structural_visitor! {
        impl<'a> StructuralRuleVisitor<'a> for RetainedExpressions<'a> {
            fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
                self.expressions.push(node);
                StructuralTraversal::Continue
            }

            fn finish(self: Box<Self>) -> Vec<Diagnostic> {
                assert_eq!(self.expressions.len(), 3);
                let Expr::Binary(binary) = self.expressions[0] else {
                    panic!("expected the parent expression before its children");
                };
                assert_eq!(binary.left().syntax().id(), self.expressions[1].syntax().id());
                assert_eq!(binary.right().syntax().id(), self.expressions[2].syntax().id());
                Vec::new()
            }
        }
    }

    #[test]
    fn collectors_retain_views_until_finish() {
        let source = "var value = 1 + 2;";
        let parsed = falcon_dart_parser::parse(source);
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors);
        let diagnostics = visit_program(
            parsed.tree.program(source),
            vec![Box::new(RetainedExpressions {
                expressions: Vec::new(),
            })],
        );
        assert_eq!(diagnostics.len(), 1);
        assert!(diagnostics[0].is_empty());
    }

    #[test]
    fn macro_subscriptions_include_exactly_the_implemented_hooks() {
        let collector = Subscribed;
        let interests = collector.interests();
        assert!(collector.has_derived_interests());
        assert_eq!(interests.enter, 1 << StructuralKind::Expr as usize);
        assert_eq!(interests.exit, 1 << StructuralKind::Program as usize);
        assert!(interests.routing);
    }

    #[test]
    fn default_subscriptions_cannot_silently_hide_callbacks() {
        let interests = StructuralInterests::default();
        for &kind in StructuralKind::ALL {
            assert!(interests.enters(kind));
            assert!(interests.exits(kind));
        }
        assert!(interests.routing);
    }
}
