//! Prefer `final` for local variable declarations that are never reassigned.
//!
//! A local declared with `var` (or just a type) that is initialized and never
//! written again reads better as `final`: the reader learns at the declaration
//! that the value does not change. Writes are tracked lexically across the
//! whole enclosing body, including closures, so a variable assigned anywhere
//! in its scope is left alone. Pattern declarations (`var (a, b) = r;`),
//! variables bound by `for (var (a, b) in pairs)`, and variable patterns in
//! `if`-case clauses, `switch` statement cases, and `switch` expression arms
//! are covered too. Like the analyzer, patterns outside any function body (in
//! top-level variable and field initializers) are not reported.
//! Local diagnostics use the AST keyword or type span, excluding matching words
//! in documentation and annotation arguments.
//!
//! ```dart
//! void f() {
//!   var count = items.length; // flagged
//!   final total = sum(items); // ok
//! }
//! ```
//!
//! Adopted from the official `prefer_final_locals` lint.

use std::collections::{HashMap, HashSet};

use falcon_analyze::{AnalyzeContext, Feature, Rule};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{
    Visitor, bound_names, walk_class_member, walk_expr, walk_local_var, walk_pattern, walk_stmt,
    walk_top_level_decl,
};

use crate::lint::lexical_usage::{find_word, local_writes};

const RULE: &str = "prefer-final-locals";
const MESSAGE: &str = "Local variables should be final.";

#[derive(Default)]
pub struct PreferFinalLocals;

impl Rule for PreferFinalLocals {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mutated: HashSet<usize> = local_writes(program)
            .into_iter()
            .map(|write| write.declaration)
            .collect();
        let mut collector = Collector {
            ctx,
            mutated,
            case_roots: HashMap::new(),
            case_variables: None,
            outside_body: false,
            diagnostics: Vec::new(),
        };
        collector.visit_program(program);
        collector.diagnostics
    }
}

/// One variable pattern of a case clause.
#[derive(Clone, Copy)]
struct CaseVariable {
    /// The whole variable pattern (`final int x`), the report span.
    span: Span,
    /// Start of the bound name, the declaration key for writes.
    name_start: usize,
    is_final: bool,
}

/// Position of a root pattern within its case clause.
#[derive(Clone, Copy)]
struct CaseRoot {
    first: bool,
    last: bool,
}

struct Collector<'a, 'ctx> {
    ctx: &'a AnalyzeContext<'ctx>,
    mutated: HashSet<usize>,
    /// Root patterns of `if`-case clauses, `switch` statement cases, and
    /// `switch` expression arms, keyed by address. Pattern declarations and
    /// pattern assignments are never registered, so their variables are not
    /// reported here.
    case_roots: HashMap<*const Pattern, CaseRoot>,
    /// Variables declared by the case clause being walked, grouped by name so
    /// joined variables (`case A(:var x) || B(:var x)`, or several `case`
    /// labels sharing one body) report once.
    case_variables: Option<Vec<(String, Vec<CaseVariable>)>>,
    diagnostics: Vec<Diagnostic>,
    /// Whether the walk is in a top-level variable or field initializer,
    /// outside any function body.
    outside_body: bool,
}

impl Collector<'_, '_> {
    fn report(&mut self, span: Span) {
        self.diagnostics
            .push(self.ctx.diagnostic(RULE, MESSAGE, span));
    }

    fn is_mutated(&self, name: &Identifier) -> bool {
        self.mutated.contains(&name.span.start)
    }

    fn register_case_clause<'p>(&mut self, roots: impl IntoIterator<Item = &'p Pattern>) {
        if self.outside_body {
            return;
        }
        let roots: Vec<&Pattern> = roots.into_iter().collect();
        let count = roots.len();
        for (index, root) in roots.into_iter().enumerate() {
            self.case_roots.insert(
                std::ptr::from_ref(root),
                CaseRoot {
                    first: index == 0,
                    last: index + 1 == count,
                },
            );
        }
    }

    fn register_if_condition(&mut self, condition: &IfCondition) {
        if let IfCondition::Case(_, pattern, _) = condition {
            self.register_case_clause([pattern.as_ref()]);
        }
    }

    /// Reports the finished case clause's unmutated, non-final variables once
    /// per joined name.
    fn finish_case_clause(&mut self) {
        let variables = self.case_variables.take().unwrap_or_default();
        for (_, declarations) in variables {
            let any_mutated = declarations
                .iter()
                .any(|variable| self.mutated.contains(&variable.name_start));
            let Some(first) = declarations.first().copied() else {
                continue;
            };
            if !any_mutated && !first.is_final {
                self.report(first.span);
            }
        }
    }

    fn register_collection_element(&mut self, element: &CollectionElement) {
        match element {
            CollectionElement::If {
                condition,
                then_elem,
                else_elem,
                ..
            } => {
                self.register_if_condition(condition);
                self.register_collection_element(then_elem);
                if let Some(else_elem) = else_elem {
                    self.register_collection_element(else_elem);
                }
            }
            CollectionElement::For {
                header, element, ..
            } => {
                self.check_for_header(header);
                self.register_collection_element(element);
            }
            CollectionElement::Expr(_)
            | CollectionElement::NullAware { .. }
            | CollectionElement::Spread { .. } => {}
        }
    }

    fn register_map_element(&mut self, element: &MapElement) {
        match element {
            MapElement::If {
                condition,
                then_entry,
                else_entry,
                ..
            } => {
                self.register_if_condition(condition);
                self.register_map_element(then_entry);
                if let Some(else_entry) = else_entry {
                    self.register_map_element(else_entry);
                }
            }
            MapElement::For { header, entry, .. } => {
                self.check_for_header(header);
                self.register_map_element(entry);
            }
            MapElement::Entry(_) | MapElement::Spread { .. } => {}
        }
    }

    fn check_for_header(&mut self, header: &ForHeader) {
        if self.outside_body {
            return;
        }
        if let Some(ForInit::PatternForIn {
            keyword, pattern, ..
        }) = &header.init
        {
            self.check_for_each_pattern(*keyword, pattern);
        }
    }

    fn check_local_var(&mut self, declaration: &LocalVarDecl) {
        if declaration.keyword != VarKeyword::Var
            || declaration.declarators.iter().any(|declarator| {
                declarator.initializer.is_none()
                    || (declarator.name.name == "_"
                        && self
                            .ctx
                            .language_version()
                            .supports(Feature::WildcardVariables))
                    || self.is_mutated(&declarator.name)
            })
        {
            return;
        }
        let Some(first) = declaration.declarators.first() else {
            return;
        };
        let span = declaration
            .keyword_span
            .or_else(|| declaration.var_type.as_ref().map(|ty| *ty.span()))
            .unwrap_or(first.name.span);
        self.report(span);
    }

    fn check_pattern_declaration(&mut self, declaration: &PatternDeclaration) {
        if declaration.keyword != VarKeyword::Var || contains_just_wildcards(&declaration.pattern) {
            return;
        }
        if bound_names(&declaration.pattern)
            .iter()
            .any(|name| self.is_mutated(name))
        {
            return;
        }
        self.report(declaration.keyword_span);
    }

    fn check_for_each_pattern(&mut self, keyword: VarKeyword, pattern: &Pattern) {
        if keyword != VarKeyword::Var {
            return;
        }
        let names = bound_names(pattern);
        if names.iter().any(|name| self.is_mutated(name)) {
            return;
        }
        for name in names {
            let span = variable_pattern_span(pattern, name).unwrap_or(name.span);
            self.report(span);
        }
    }
}

impl Visitor for Collector<'_, '_> {
    fn visit_top_level_decl(&mut self, node: &TopLevelDecl) {
        let outside = std::mem::replace(
            &mut self.outside_body,
            matches!(node, TopLevelDecl::Variable(_)),
        );
        walk_top_level_decl(self, node);
        self.outside_body = outside;
    }

    fn visit_class_member(&mut self, node: &ClassMember) {
        let outside = std::mem::replace(
            &mut self.outside_body,
            matches!(node, ClassMember::Field(_)),
        );
        walk_class_member(self, node);
        self.outside_body = outside;
    }

    fn visit_local_var(&mut self, node: &LocalVarDecl) {
        self.check_local_var(node);
        walk_local_var(self, node);
    }

    fn visit_stmt(&mut self, node: &Stmt) {
        if let Stmt::PatternDecl(declaration) = node {
            self.check_pattern_declaration(declaration);
        } else if let Stmt::For(statement) = node {
            self.check_for_header(&statement.header);
        } else if let Stmt::If(statement) = node {
            self.register_if_condition(&statement.condition);
        } else if let Stmt::Switch(statement) = node {
            for case in &statement.cases {
                self.register_case_clause(case.cases.iter().filter_map(|kind| match kind {
                    SwitchCaseKind::Pattern(pattern, _) => Some(pattern.as_ref()),
                    SwitchCaseKind::Expr(_) | SwitchCaseKind::Default => None,
                }));
            }
        }
        walk_stmt(self, node);
    }

    fn visit_expr(&mut self, node: &Expr) {
        // Collection `if`-case elements are case clauses and collection `for`
        // elements are for-each loops.
        if let Expr::List { elements, .. } | Expr::Set { elements, .. } = node {
            for element in elements {
                self.register_collection_element(element);
            }
        } else if let Expr::Map { elements, .. } = node {
            for element in elements {
                self.register_map_element(element);
            }
        } else if let Expr::Switch { arms, .. } = node {
            for arm in arms {
                self.register_case_clause([&arm.pattern]);
            }
        } else if let Expr::FuncExpr { .. } = node {
            let outside = std::mem::replace(&mut self.outside_body, false);
            walk_expr(self, node);
            self.outside_body = outside;
            return;
        }
        walk_expr(self, node);
    }

    fn visit_pattern(&mut self, node: &Pattern) {
        let root = self.case_roots.remove(&std::ptr::from_ref(node));
        if root.is_some_and(|root| root.first) {
            self.case_variables = Some(Vec::new());
        }
        if let Pattern::Variable { name, span, .. } = node
            && let Some(variables) = self.case_variables.as_mut()
        {
            let is_final = find_word(self.ctx.source, span.start, name.span.start, "final")
                .is_some_and(|keyword| keyword.start == span.start);
            let variable = CaseVariable {
                span: *span,
                name_start: name.span.start,
                is_final,
            };
            match variables
                .iter_mut()
                .find(|(existing, _)| *existing == name.name)
            {
                Some((_, declarations)) => declarations.push(variable),
                None => variables.push((name.name.clone(), vec![variable])),
            }
        }
        walk_pattern(self, node);
        if root.is_some_and(|root| root.last) {
            self.finish_case_clause();
        }
    }
}

/// The span of the `Pattern::Variable` declaring `name` inside `pattern`.
fn variable_pattern_span(pattern: &Pattern, name: &Identifier) -> Option<Span> {
    let mut found = None;
    let mut finder = VariableFinder {
        target: name.span,
        found: &mut found,
    };
    finder.visit_pattern(pattern);
    found
}

struct VariableFinder<'a> {
    target: Span,
    found: &'a mut Option<Span>,
}

impl Visitor for VariableFinder<'_> {
    fn visit_pattern(&mut self, node: &Pattern) {
        if let Pattern::Variable { name, span, .. } = node
            && name.span == self.target
        {
            *self.found = Some(*span);
        }
        walk_pattern(self, node);
    }
}

/// `var (_, _) = r;` declares nothing, so there is nothing to make final.
fn contains_just_wildcards(pattern: &Pattern) -> bool {
    match pattern {
        Pattern::Wildcard { .. } => true,
        Pattern::List(list) => list.elements.iter().all(|element| match element {
            ListPatternElement::Pattern(pattern) => contains_just_wildcards(pattern),
            ListPatternElement::Rest(..) => false,
        }),
        Pattern::Map(map) => map
            .entries
            .iter()
            .all(|entry| matches!(entry.pattern, Pattern::Wildcard { .. })),
        Pattern::Object(object) => object
            .fields
            .iter()
            .all(|field| field.pattern.as_ref().is_some_and(contains_just_wildcards)),
        Pattern::Record(record) => record
            .fields
            .iter()
            .all(|field| contains_just_wildcards(&field.pattern)),
        Pattern::ParenPattern { inner, .. } => contains_just_wildcards(inner),
        Pattern::Variable { .. }
        | Pattern::Literal(_)
        | Pattern::Const(_)
        | Pattern::LogicalAnd { .. }
        | Pattern::LogicalOr { .. }
        | Pattern::Relational { .. }
        | Pattern::Cast { .. }
        | Pattern::NullCheck { .. }
        | Pattern::NullAssert { .. }
        | Pattern::Error { .. } => false,
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use falcon_config::FalconConfig;
    use falcon_dart_parser::parse;

    use super::*;

    fn lines(source: &str) -> Vec<usize> {
        let (program, errors) = parse(source);
        assert!(errors.is_empty(), "{errors:?}");
        let config = FalconConfig::default();
        let ctx = AnalyzeContext::new(Path::new("a.dart"), source, &config);
        PreferFinalLocals
            .analyze(&program, &ctx)
            .iter()
            .map(|d| d.span.start)
            .collect()
    }

    #[test]
    fn closure_write_counts_as_mutation() {
        assert!(lines("void f() { var a = 1; void g() { a = 2; } g(); }").is_empty());
    }

    #[test]
    fn shadowing_closure_parameter_is_not_a_write() {
        assert_eq!(
            lines("void f() { var a = 1; (int a) { a = 2; }; print(a); }").len(),
            1
        );
    }

    #[test]
    fn local_keyword_and_type_spans_ignore_var_in_documentation_and_metadata() {
        let source = r#"void f() {
  /// var belongs to the documentation.
  @pragma('var')
  var inferred = 1;
  /// var is not the keyword of this typed declaration.
  @pragma('var')
  int typed = 2;
  /// var belongs to the pattern documentation.
  @pragma('var')
  var [first, second] = [inferred, typed];
  print(first + second);
}"#;
        let (program, errors) = parse(source);
        assert!(errors.is_empty(), "{errors:?}");
        let config = FalconConfig::default();
        let ctx = AnalyzeContext::new(Path::new("test.dart"), source, &config);
        let diagnostics = PreferFinalLocals.analyze(&program, &ctx);
        assert_eq!(diagnostics.len(), 3, "{diagnostics:?}");
        let keyword = source.find("var inferred").unwrap();
        let ty = source.find("int typed").unwrap();
        let pattern = source.find("var [first").unwrap();
        assert_eq!(diagnostics[0].span, Span::new(keyword, keyword + 3).into());
        assert_eq!(diagnostics[1].span, Span::new(ty, ty + 3).into());
        assert_eq!(diagnostics[2].span, Span::new(pattern, pattern + 3).into());
    }
}
