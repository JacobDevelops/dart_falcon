//! Shared source-order collectors, driven exclusively by the syntax Visitor walkers.
//!
//! Collectors own their context and diagnostics. Every delivered enter has a matching
//! exit, including `SkipChildren`; a rejected child has neither. Pruning and routing
//! affect only that collector, never the physical traversal or another collector.

use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{self, Visitor};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StructuralTraversal {
    Continue,
    SkipChildren,
}

macro_rules! structural_nodes {
    ($lifetime:lifetime; $(
        $kind:ident, $visit:ident, $exit:ident, $ty:ty, [$($walk:ident)?];
    )*) => {
        /// A node delivered by the existing structural Visitor contract.
        #[derive(Clone, Copy)]
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

        /// An independent collector; callbacks must not recurse into the AST.
        ///
        /// Use [`crate::structural_visitor!`] to derive subscriptions from the hooks.
        /// A direct implementation defaults to all hooks and remains correct.
        pub trait StructuralRuleVisitor {
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
                _parent: StructuralNode<'_>,
                _child: StructuralNode<'_>,
            ) -> bool {
                true
            }

            $(
                fn $visit<$lifetime>(&mut self, _node: $ty) -> StructuralTraversal {
                    StructuralTraversal::Continue
                }

                fn $exit<$lifetime>(&mut self, _node: $ty) {}
            )*
        }

        impl StructuralInterests {
            /// Macro support; derive subscriptions from the implemented method names.
            #[doc(hidden)]
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

            let source = include_str!("../../falcon_syntax/src/visitor.rs");
            let trait_body = source.split_once("pub trait Visitor: Sized {").unwrap().1;
            let trait_body = trait_body.split_once("\n}").unwrap().0;
            let syntax_hooks: BTreeSet<_> = trait_body.lines().filter_map(|line| {
                line.trim_start().strip_prefix("fn ")
                    .map(|signature| signature.split('(').next().unwrap())
            }).collect();
            let structural_hooks: BTreeSet<_> = [$(stringify!($visit),)*].into_iter().collect();
            assert_eq!(
                structural_hooks, syntax_hooks,
                "every syntax Visitor hook must have structural enter/exit, node and dispatch entries"
            );
        }

        impl Visitor for StructuralVisitors<'_, '_, '_> {
            $(
                fn $visit<$lifetime>(&mut self, node: $ty) {
                    let kind = StructuralKind::$kind;
                    let child = StructuralNode::$kind(node);
                    for &index in self.subscribers.for_kind(kind) {
                        let collector = &mut self.collectors[index];
                        if collector.suppressed.is_some() {
                            continue;
                        }
                        if collector.interests.routing
                            && let Some(parent) = self.parent
                            && !collector.visitor.route_child(parent, child)
                        {
                            collector.suppressed = Some((self.depth, false));
                            continue;
                        }
                        if collector.interests.enters(kind)
                            && collector.visitor.$visit(node) == StructuralTraversal::SkipChildren
                        {
                            collector.suppressed = Some((self.depth, true));
                        }
                    }
                    $(
                        let mut children = StructuralVisitors {
                            collectors: self.collectors,
                            subscribers: self.subscribers,
                            parent: Some(child),
                            depth: self.depth + 1,
                        };
                        visitor::$walk(&mut children, node);
                    )?
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
                            collector.visitor.$exit(node);
                        }
                    }
                }
            )*
        }
    };
}

structural_nodes! {
    'n;
    Program, visit_program, exit_program, &'n Program, [walk_program];
    TypeAliasDecl, visit_type_alias_decl, exit_type_alias_decl, &'n TypeAliasDecl, [walk_type_alias_decl];
    Import, visit_import, exit_import, &'n ImportDirective, [walk_import];
    Export, visit_export, exit_export, &'n ExportDirective, [walk_export];
    TopLevelDecl, visit_top_level_decl, exit_top_level_decl, &'n TopLevelDecl, [walk_top_level_decl];
    ClassDecl, visit_class_decl, exit_class_decl, &'n ClassDecl, [walk_class_decl];
    MixinDecl, visit_mixin_decl, exit_mixin_decl, &'n MixinDecl, [walk_mixin_decl];
    EnumDecl, visit_enum_decl, exit_enum_decl, &'n EnumDecl, [walk_enum_decl];
    ExtensionDecl, visit_extension_decl, exit_extension_decl, &'n ExtensionDecl, [walk_extension_decl];
    FunctionDecl, visit_function_decl, exit_function_decl, &'n FunctionDecl, [walk_function_decl];
    ClassMember, visit_class_member, exit_class_member, &'n ClassMember, [walk_class_member];
    FieldDecl, visit_field_decl, exit_field_decl, &'n FieldDecl, [walk_field_decl];
    Constructor, visit_constructor, exit_constructor, ConstructorRef<'n>, [];
    ConstructorDecl, visit_constructor_decl, exit_constructor_decl, &'n ConstructorDecl, [walk_constructor_decl];
    MethodDecl, visit_method_decl, exit_method_decl, &'n MethodDecl, [walk_method_decl];
    GetterDecl, visit_getter_decl, exit_getter_decl, &'n GetterDecl, [walk_getter_decl];
    SetterDecl, visit_setter_decl, exit_setter_decl, &'n SetterDecl, [walk_setter_decl];
    DartType, visit_dart_type, exit_dart_type, &'n DartType, [walk_dart_type];
    Stmt, visit_stmt, exit_stmt, &'n Stmt, [walk_stmt];
    Block, visit_block, exit_block, &'n Block, [walk_block];
    FunctionBody, visit_function_body, exit_function_body, &'n FunctionBody, [walk_function_body];
    SwitchCase, visit_switch_case, exit_switch_case, &'n SwitchCase, [walk_switch_case];
    CatchClause, visit_catch_clause, exit_catch_clause, &'n CatchClause, [walk_catch_clause];
    LocalVar, visit_local_var, exit_local_var, &'n LocalVarDecl, [walk_local_var];
    ForHeader, visit_for_header, exit_for_header, &'n ForHeader, [walk_for_header];
    Expr, visit_expr, exit_expr, &'n Expr, [walk_expr];
    CollectionElement, visit_collection_element, exit_collection_element, &'n CollectionElement, [walk_collection_element];
    MapElement, visit_map_element, exit_map_element, &'n MapElement, [walk_map_element];
    SwitchExprArm, visit_switch_expr_arm, exit_switch_expr_arm, &'n SwitchExprArm, [walk_switch_expr_arm];
    Pattern, visit_pattern, exit_pattern, &'n Pattern, [walk_pattern];
    Annotation, visit_annotation, exit_annotation, &'n Annotation, [walk_annotation];
    Identifier, visit_identifier, exit_identifier, &'n Identifier, [];
    StringLit, visit_string_lit, exit_string_lit, &'n StringLitNode, [];
    FormalParam, visit_formal_param, exit_formal_param, &'n FormalParam, [walk_formal_param];
    PrimaryConstructor, visit_primary_constructor, exit_primary_constructor, &'n PrimaryConstructor, [walk_primary_constructor];
    PrimaryConstructorBody, visit_primary_constructor_body, exit_primary_constructor_body, &'n PrimaryConstructorBody, [walk_primary_constructor_body];
    LibraryDirective, visit_library_directive, exit_library_directive, &'n LibraryDirective, [walk_library_directive];
    PartOfDirective, visit_part_of_directive, exit_part_of_directive, &'n PartOfDirective, [walk_part_of_directive];
    PartDirective, visit_part_directive, exit_part_directive, &'n PartDirective, [walk_part_directive];
    ConfigurableUri, visit_configurable_uri, exit_configurable_uri, &'n ConfigurableUri, [walk_configurable_uri];
    ImportCombinator, visit_import_combinator, exit_import_combinator, &'n ImportCombinator, [walk_import_combinator];
    ClassTypeAlias, visit_class_type_alias, exit_class_type_alias, &'n ClassTypeAliasDecl, [walk_class_type_alias];
    MixinClass, visit_mixin_class, exit_mixin_class, &'n MixinClassDecl, [walk_mixin_class];
    ExtensionType, visit_extension_type, exit_extension_type, &'n ExtensionTypeDecl, [walk_extension_type];
    TopLevelVar, visit_top_level_var, exit_top_level_var, &'n TopLevelVarDecl, [walk_top_level_var];
    OperatorDecl, visit_operator_decl, exit_operator_decl, &'n OperatorDecl, [walk_operator_decl];
    ConstructorInitializer, visit_constructor_initializer, exit_constructor_initializer, &'n ConstructorInitializer, [walk_constructor_initializer];
    ArgList, visit_arg_list, exit_arg_list, &'n ArgList, [walk_arg_list];
    CascadeSection, visit_cascade_section, exit_cascade_section, &'n CascadeSection, [walk_cascade_section];
    ForInit, visit_for_init, exit_for_init, &'n ForInit, [walk_for_init];
    EnumVariant, visit_enum_variant, exit_enum_variant, &'n EnumVariant, [walk_enum_variant];
    VarDeclarator, visit_var_declarator, exit_var_declarator, &'n VarDeclarator, [walk_var_declarator];
    RedirectedConstructor, visit_redirected_constructor, exit_redirected_constructor, &'n RedirectedConstructor, [walk_redirected_constructor];
    TypeParam, visit_type_param, exit_type_param, &'n TypeParam, [walk_type_param];
    FormalParamList, visit_formal_param_list, exit_formal_param_list, &'n FormalParamList, [walk_formal_param_list];
    NamedType, visit_named_type, exit_named_type, &'n NamedType, [walk_named_type];
    FunctionType, visit_function_type, exit_function_type, &'n FunctionType, [walk_function_type];
    FunctionTypeParam, visit_function_type_param, exit_function_type_param, &'n FunctionTypeParam, [walk_function_type_param];
    RecordType, visit_record_type, exit_record_type, &'n RecordType, [walk_record_type];
    NamedRecordField, visit_named_record_field, exit_named_record_field, &'n NamedRecordField, [walk_named_record_field];
    IfCondition, visit_if_condition, exit_if_condition, &'n IfCondition, [walk_if_condition];
    IfStmt, visit_if_stmt, exit_if_stmt, &'n IfStmt, [walk_if_stmt];
    ForStmt, visit_for_stmt, exit_for_stmt, &'n ForStmt, [walk_for_stmt];
    WhileStmt, visit_while_stmt, exit_while_stmt, &'n WhileStmt, [walk_while_stmt];
    DoWhileStmt, visit_do_while_stmt, exit_do_while_stmt, &'n DoWhileStmt, [walk_do_while_stmt];
    SwitchStmt, visit_switch_stmt, exit_switch_stmt, &'n SwitchStmt, [walk_switch_stmt];
    TryCatchStmt, visit_try_catch_stmt, exit_try_catch_stmt, &'n TryCatchStmt, [walk_try_catch_stmt];
    ReturnStmt, visit_return_stmt, exit_return_stmt, &'n ReturnStmt, [walk_return_stmt];
    ThrowStmt, visit_throw_stmt, exit_throw_stmt, &'n ThrowStmt, [walk_throw_stmt];
    BreakStmt, visit_break_stmt, exit_break_stmt, &'n BreakStmt, [walk_break_stmt];
    ContinueStmt, visit_continue_stmt, exit_continue_stmt, &'n ContinueStmt, [walk_continue_stmt];
    PatternDeclaration, visit_pattern_declaration, exit_pattern_declaration, &'n PatternDeclaration, [walk_pattern_declaration];
    PatternAssignStmt, visit_pattern_assign_stmt, exit_pattern_assign_stmt, &'n PatternAssignStmt, [walk_pattern_assign_stmt];
    LabeledStmt, visit_labeled_stmt, exit_labeled_stmt, &'n LabeledStmt, [walk_labeled_stmt];
    LocalFuncDecl, visit_local_func_decl, exit_local_func_decl, &'n LocalFuncDecl, [walk_local_func_decl];
    AssertStmt, visit_assert_stmt, exit_assert_stmt, &'n AssertStmt, [walk_assert_stmt];
    YieldStmt, visit_yield_stmt, exit_yield_stmt, &'n YieldStmt, [walk_yield_stmt];
    ExprStmt, visit_expr_stmt, exit_expr_stmt, &'n ExprStmt, [walk_expr_stmt];
    ErrorNode, visit_error_node, exit_error_node, &'n ErrorNode, [];
    SwitchCaseKind, visit_switch_case_kind, exit_switch_case_kind, &'n SwitchCaseKind, [walk_switch_case_kind];
    CascadeOp, visit_cascade_op, exit_cascade_op, &'n CascadeOp, [walk_cascade_op];
    MapEntry, visit_map_entry, exit_map_entry, &'n MapEntry, [walk_map_entry];
    RecordField, visit_record_field, exit_record_field, &'n RecordField, [walk_record_field];
    NamedArg, visit_named_arg, exit_named_arg, &'n NamedArg, [walk_named_arg];
    StringInterpolation, visit_string_interpolation, exit_string_interpolation, &'n StringInterpolation, [walk_string_interpolation];
    LiteralPattern, visit_literal_pattern, exit_literal_pattern, &'n LiteralPattern, [walk_literal_pattern];
    LiteralPatternValue, visit_literal_pattern_value, exit_literal_pattern_value, &'n LiteralPatternValue, [walk_literal_pattern_value];
    ConstPattern, visit_const_pattern, exit_const_pattern, &'n ConstPattern, [walk_const_pattern];
    ConstPatternValue, visit_const_pattern_value, exit_const_pattern_value, &'n ConstPatternValue, [walk_const_pattern_value];
    ListPattern, visit_list_pattern, exit_list_pattern, &'n ListPattern, [walk_list_pattern];
    ListPatternElement, visit_list_pattern_element, exit_list_pattern_element, &'n ListPatternElement, [walk_list_pattern_element];
    RecordPattern, visit_record_pattern, exit_record_pattern, &'n RecordPattern, [walk_record_pattern];
    RecordPatternField, visit_record_pattern_field, exit_record_pattern_field, &'n RecordPatternField, [walk_record_pattern_field];
    MapPattern, visit_map_pattern, exit_map_pattern, &'n MapPattern, [walk_map_pattern];
    MapPatternEntry, visit_map_pattern_entry, exit_map_pattern_entry, &'n MapPatternEntry, [walk_map_pattern_entry];
    ObjectPattern, visit_object_pattern, exit_object_pattern, &'n ObjectPattern, [walk_object_pattern];
    ObjectPatternField, visit_object_pattern_field, exit_object_pattern_field, &'n ObjectPatternField, [walk_object_pattern_field];
    FunctionKind, visit_function_kind, exit_function_kind, &'n FunctionKind, [walk_function_kind];
    Name, visit_name, exit_name, &'n Identifier, [];
    NativeString, visit_native_string, exit_native_string, &'n StringLitNode, [];
}

/// Implement a structural collector and derive subscriptions from its hooks.
///
/// Wrap the complete implementation, including `finish` and `route_child`.
/// Do not declare `interests` or `has_derived_interests` manually.
///
/// ```
/// use falcon_analyze::{StructuralRuleVisitor, StructuralTraversal};
/// use falcon_diagnostics::Diagnostic;
/// use falcon_syntax::ast::Expr;
///
/// struct Collector {
///     diags: Vec<Diagnostic>,
/// }
///
/// falcon_analyze::structural_visitor! {
///     impl StructuralRuleVisitor for Collector {
///         fn visit_expr(&mut self, _node: &Expr) -> StructuralTraversal {
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
        impl $(<$($lifetime:lifetime),*>)? $visitor:ident for $collector:ty {
            $(
                $(#[$attribute:meta])*
                fn $method:ident $args:tt $(-> $result:ty)? $body:block
            )*
        }
    ) => {
        impl $(<$($lifetime),*>)? $visitor for $collector {
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

    pub const fn new() -> Self {
        Self {
            enter: 0,
            exit: 0,
            routing: false,
        }
    }

    pub const fn all() -> Self {
        Self {
            enter: u128::MAX,
            exit: u128::MAX,
            routing: true,
        }
    }

    pub const fn enter(mut self, kind: StructuralKind) -> Self {
        self.enter |= 1 << kind as usize;
        self
    }

    pub const fn exit(mut self, kind: StructuralKind) -> Self {
        self.exit |= 1 << kind as usize;
        self
    }

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
    visitor: Box<dyn StructuralRuleVisitor + 'a>,
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

struct StructuralVisitors<'v, 'n, 'a> {
    collectors: &'v mut [Collector<'a>],
    subscribers: &'v Subscriptions,
    parent: Option<StructuralNode<'n>>,
    depth: usize,
}

/// Run one existing Visitor traversal and consume every collector in input order.
/// Subscription tables and collector state allocate only once per program.
pub(crate) fn visit_program<'a>(
    program: &Program,
    visitors: Vec<Box<dyn StructuralRuleVisitor + 'a>>,
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

    struct Subscribed;

    crate::structural_visitor! {
        impl StructuralRuleVisitor for Subscribed {
            fn visit_expr(&mut self, _node: &Expr) -> StructuralTraversal {
                StructuralTraversal::Continue
            }

            fn exit_program(&mut self, _node: &Program) {}

            fn route_child(&mut self, _parent: StructuralNode<'_>, _child: StructuralNode<'_>) -> bool {
                true
            }

            fn finish(self: Box<Self>) -> Vec<Diagnostic> {
                Vec::new()
            }
        }
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
