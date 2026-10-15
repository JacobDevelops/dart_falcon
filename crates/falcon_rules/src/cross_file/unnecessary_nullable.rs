//! Report nullable parameters of private declarations that never receive null.
//!
//! Flags a nullable parameter (`T?`) of a private (`_`-prefixed) function or
//! method when no call site anywhere in the project passes it null, and its body
//! never assigns null to it either. Because the declaration is private, every
//! one of its call sites is visible in the analyzed set, which is what makes the
//! conclusion sound; the `?` then widens the type for a value that is always
//! non-null, so callers and the body carry null handling that can never trigger.
//! An argument counts as non-null only when proven so — a literal, a `new` or
//! arithmetic expression, or a call whose declared return type the cross-file
//! index resolves as non-nullable. Anything uncertain (an ambiguous name shared
//! by multiple declarations, a tear-off reference, an argument that might be
//! null) suppresses the report. This is a cross-file rule: it runs in the
//! cross-file pass over the whole analyzed file set and is configured under the
//! top-level `cross-file` section rather than `linter`.

use std::collections::{HashMap, HashSet};

use crate::lint::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};
use falcon_analyze::{
    AnalyzeContext, CrossFileRule, IdentityIndex, IdentitySource, LibrarySource, ProjectFile,
    SemanticIndexes, SignatureIndex, TypeIndex, group_libraries,
};
use falcon_config::FalconConfig;
use falcon_diagnostics::{Diagnostic, Severity};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr, walk_function_body};

#[derive(Default)]
pub struct UnnecessaryNullable;

const NAME: &str = "unnecessary-nullable";

impl CrossFileRule for UnnecessaryNullable {
    fn analyze_project(
        &self,
        files: &[ProjectFile],
        config: &FalconConfig,
        indexes: Option<&SemanticIndexes>,
    ) -> Vec<Diagnostic> {
        // Count every function/method by name project-wide so we only reason
        // about names that resolve to exactly one declaration.
        let mut decl_count: HashMap<String, usize> = HashMap::new();
        for f in files {
            if f.has_parse_errors {
                continue;
            }
            for name in decl_names(&f.program) {
                *decl_count.entry(name).or_default() += 1;
            }
        }

        let owned_indexes;
        let indexes = match indexes {
            Some(indexes) => indexes,
            None => {
                let project_files: Vec<_> = files
                    .iter()
                    .map(|file| (file.path.clone(), &file.program))
                    .collect();
                let grouping = group_libraries(&project_files);
                let sources: Vec<_> = files
                    .iter()
                    .map(|file| IdentitySource {
                        path: &file.path,
                        program: &file.program,
                        has_parse_errors: file.has_parse_errors,
                    })
                    .collect();
                let packages =
                    crate::package_identities(files.iter().map(|file| file.path.as_path()));
                let identities = IdentityIndex::from_project_files(&sources, &packages);
                let types = TypeIndex::from_resolved_files(
                    files.iter().enumerate().map(|(index, file)| {
                        (
                            file.path.as_path(),
                            LibrarySource {
                                program: &file.program,
                                has_parse_errors: file.has_parse_errors,
                                has_unresolved_parts: grouping.is_unresolved(index),
                            },
                        )
                    }),
                    &identities,
                );
                let signatures =
                    SignatureIndex::from_project_files(&project_files, &identities, &types);
                owned_indexes = SemanticIndexes {
                    types,
                    identities,
                    signatures,
                };
                &owned_indexes
            }
        };
        let mut calls = Vec::new();
        let mut tear_offs = HashSet::new();
        for file in files {
            if file
                .path
                .extension()
                .and_then(|extension| extension.to_str())
                != Some("dart")
            {
                continue;
            }
            let ctx = AnalyzeContext::new(&file.path, &file.source, config)
                .with_types(&indexes.types)
                .with_identities(&indexes.identities)
                .with_signatures(&indexes.signatures);
            let mut members = MemberBindings(HashSet::new());
            members.visit_program(&file.program);
            let mut nullability = ArgumentNullability {
                values: HashMap::new(),
                members: members.0,
            };
            let state = SemanticState::new(&file.program, &ctx)
                .expect("project identities describe the analyzed files");
            visit_program(&mut nullability, &file.program, state);
            let mut collector = CallCollector {
                calls: Vec::new(),
                tear_offs: HashSet::new(),
                nullability: &nullability.values,
            };
            collector.visit_program(&file.program);
            calls.extend(collector.calls);
            tear_offs.extend(collector.tear_offs);
        }

        let mut diags = Vec::new();
        for f in files {
            if f.has_parse_errors {
                continue;
            }
            for target in private_targets(&f.program) {
                if decl_count.get(&target.name).copied() != Some(1) {
                    continue; // ambiguous name — can't attribute call sites
                }
                // Used as a value (tear-off): callers are opaque, so bail.
                if tear_offs.contains(&target.name) {
                    continue;
                }
                let calls: Vec<&CallInfo> =
                    calls.iter().filter(|c| c.name == target.name).collect();
                if calls.is_empty() {
                    continue; // no evidence of how it's called
                }
                let assigned_null = body_null_assignments(target.body);
                for param in &target.nullable_params {
                    if assigned_null.contains(&param.name) {
                        continue;
                    }
                    if calls.iter().any(|c| c.passes_null(param)) {
                        continue;
                    }
                    diags.push(Diagnostic::new(
                        NAME,
                        Severity::Warning,
                        format!(
                            "Parameter '{}' is never null at any call site; \
                             the '?' is unnecessary",
                            param.name
                        ),
                        f.path.to_string_lossy().into_owned(),
                        param.span,
                    ));
                }
            }
        }
        diags
    }
}

/// Where a nullable parameter sits in a call's argument list.
enum ParamSlot {
    /// Index among positional parameters; `optional` marks an optional
    /// positional parameter, which a call may legally omit.
    Positional { index: usize, optional: bool },
    /// Named parameter; `optional` when it is not `required`.
    Named { optional: bool },
}

/// A nullable parameter of a private declaration.
struct NullableParam {
    name: String,
    span: Span,
    slot: ParamSlot,
    /// True when an omitted optional param would default to null (no non-null
    /// default is written), so omission is treated as passing null.
    default_is_null: bool,
}

/// A private top-level function or method under consideration.
struct Target<'a> {
    name: String,
    nullable_params: Vec<NullableParam>,
    body: Option<&'a FunctionBody>,
}

/// Collect the private functions/methods of a program with their nullable
/// parameters. Only `_`-prefixed names are considered.
fn private_targets(program: &Program) -> Vec<Target<'_>> {
    let mut out = Vec::new();
    for decl in &program.declarations {
        match decl {
            TopLevelDecl::Function(f)
                if is_private(&f.name.name)
                    && matches!(f.kind, falcon_syntax::ast::FunctionKind::Function(_)) =>
            {
                if let Some(params) = f.params() {
                    push_target(&mut out, &f.name.name, params, f.body.as_ref());
                }
            }
            _ => {
                if let Some(members) = crate::lint::declarations::members_of(decl) {
                    collect_method_targets(&mut out, members);
                }
            }
        }
    }
    out
}

fn collect_method_targets<'a>(out: &mut Vec<Target<'a>>, members: &'a [ClassMember]) {
    for member in members {
        if let ClassMember::Method(m) = member
            && is_private(&m.name.name)
        {
            push_target(out, &m.name.name, &m.params, m.body.as_ref());
        }
    }
}

fn push_target<'a>(
    out: &mut Vec<Target<'a>>,
    name: &str,
    params: &FormalParamList,
    body: Option<&'a FunctionBody>,
) {
    let nullable_params = nullable_params(params);
    if nullable_params.is_empty() {
        return;
    }
    out.push(Target {
        name: name.to_string(),
        nullable_params,
        body,
    });
}

/// Extract the nullable parameters of a formal list with their call-site slots.
fn nullable_params(params: &FormalParamList) -> Vec<NullableParam> {
    let mut out = Vec::new();
    for (i, p) in params.positional().enumerate() {
        if let Some(np) = make_param(
            p,
            ParamSlot::Positional {
                index: i,
                optional: matches!(p.kind, falcon_syntax::ast::ParamKind::OptionalPositional),
            },
        ) {
            out.push(np);
        }
    }
    for p in params.named() {
        if let Some(np) = make_param(
            p,
            ParamSlot::Named {
                optional: matches!(
                    p.kind,
                    falcon_syntax::ast::ParamKind::Named { required: false }
                ),
            },
        ) {
            out.push(np);
        }
    }
    out
}

fn make_param(p: &FormalParam, slot: ParamSlot) -> Option<NullableParam> {
    // Skip constructor field/super params and function-typed params — out of
    // scope; only a written nullable type qualifies.
    if p.initializing.is_some() || p.function_params.is_some() {
        return None;
    }
    let ty = p.param_type.as_ref()?;
    if !ty.is_nullable() {
        return None;
    }
    let default_is_null = match p.default_value.as_ref().map(Expr::unparenthesized) {
        None => true,
        Some(Expr::NullLit { .. }) => true,
        Some(_) => false,
    };
    Some(NullableParam {
        name: p.name.name.clone(),
        span: p.name.span,
        slot,
        default_is_null,
    })
}

fn is_private(name: &str) -> bool {
    name.starts_with('_')
}

/// Names of top-level functions and methods (for the ambiguity count).
fn decl_names(program: &Program) -> Vec<String> {
    let mut out = Vec::new();
    for decl in &program.declarations {
        match decl {
            TopLevelDecl::Function(f)
                if !matches!(f.kind, falcon_syntax::ast::FunctionKind::Getter)
                    && !matches!(f.kind, falcon_syntax::ast::FunctionKind::Setter(_)) =>
            {
                out.push(f.name.name.clone())
            }
            _ => {
                if let Some(members) = crate::lint::declarations::members_of(decl) {
                    method_names(&mut out, members);
                }
            }
        }
    }
    out
}

fn method_names(out: &mut Vec<String>, members: &[ClassMember]) {
    for member in members {
        if let ClassMember::Method(m) = member {
            out.push(m.name.name.clone());
        }
    }
}

// ── Call-site collection ──────────────────────────────────────────────────────

/// A single call's argument shape relevant to nullability.
struct CallInfo {
    name: String,
    positional_maybe_null: Vec<bool>,
    named_maybe_null: HashMap<String, bool>,
}

impl CallInfo {
    /// Whether this call *might* pass null (or omit-as-null) for `param`. A `true`
    /// here suppresses the diagnostic — the rule only flags a param when every
    /// call is proven to pass a non-null value.
    fn passes_null(&self, param: &NullableParam) -> bool {
        match &param.slot {
            ParamSlot::Positional { index, optional } => {
                match self.positional_maybe_null.get(*index) {
                    Some(maybe_null) => *maybe_null,
                    // Argument omitted: for an optional param that defaults to null,
                    // omission means null; a required param can't legally be omitted.
                    None => *optional && param.default_is_null,
                }
            }
            ParamSlot::Named { optional } => match self.named_maybe_null.get(&param.name) {
                Some(maybe_null) => *maybe_null,
                None => *optional && param.default_is_null,
            },
        }
    }
}

fn is_null_expr(e: &Expr) -> bool {
    matches!(e.unparenthesized(), Expr::NullLit { .. })
}

fn call_info(name: String, args: &ArgList, collector: &CallCollector<'_>) -> CallInfo {
    let maybe_null = |arg: &Expr| {
        let span = arg.span();
        collector
            .nullability
            .get(&(span.start, span.end))
            .copied()
            .unwrap_or(true)
    };
    CallInfo {
        name,
        positional_maybe_null: args.positional.iter().map(maybe_null).collect(),
        named_maybe_null: args
            .named
            .iter()
            .map(|arg| (arg.name.name.clone(), maybe_null(&arg.value)))
            .collect(),
    }
}

fn callee_name(callee: &Expr) -> Option<String> {
    match callee.unparenthesized() {
        Expr::Ident(id) => Some(id.name.clone()),
        Expr::Field { field, .. } => Some(field.name.clone()),
        _ => None,
    }
}

struct CallCollector<'a> {
    calls: Vec<CallInfo>,
    /// Names used as bare values (tear-offs), not as a call callee.
    tear_offs: HashSet<String>,
    nullability: &'a HashMap<(usize, usize), bool>,
}

struct ArgumentNullability {
    values: HashMap<(usize, usize), bool>,
    members: HashSet<String>,
}

impl ArgumentNullability {
    fn record(&mut self, args: &ArgList, state: &SemanticState<'_>) {
        for arg in args
            .positional
            .iter()
            .chain(args.named.iter().map(|arg| &arg.value))
        {
            let span = arg.span();
            let mut shadow = MemberReference {
                names: &self.members,
                found: false,
            };
            if state.enclosing_type.is_some() && !self.members.is_empty() {
                shadow.visit_expr(arg);
            }
            self.values.insert(
                (span.start, span.end),
                shadow.found || state.infer(arg).nullable(),
            );
        }
    }
}

struct MemberBindings(HashSet<String>);

impl Visitor for MemberBindings {
    fn visit_class_member(&mut self, member: &ClassMember) {
        match member {
            ClassMember::Method(method) => {
                self.0.insert(method.name.name.clone());
            }
            ClassMember::Getter(getter) => {
                self.0.insert(getter.name.name.clone());
            }
            ClassMember::Setter(setter) => {
                self.0.insert(setter.name.name.clone());
            }
            ClassMember::Field(field) => {
                self.0.extend(
                    field
                        .declarators
                        .iter()
                        .map(|declarator| declarator.name.name.clone()),
                );
            }
            _ => {}
        }
        falcon_syntax::visitor::walk_class_member(self, member);
    }
}

struct MemberReference<'a> {
    names: &'a HashSet<String>,
    found: bool,
}

impl Visitor for MemberReference<'_> {
    fn visit_expr(&mut self, expr: &Expr) {
        if self.found {
            return;
        }
        if let Expr::Ident(identifier) = expr
            && self.names.contains(&identifier.name)
        {
            self.found = true;
        } else {
            walk_expr(self, expr);
        }
    }
}

impl SemanticRuleVisitor for ArgumentNullability {
    fn visit_expr(&mut self, expr: &Expr, state: &SemanticState<'_>) {
        match expr {
            Expr::Call { args, .. } => self.record(args, state),
            Expr::Cascade { sections, .. } => {
                for section in sections {
                    for op in &section.ops {
                        if let CascadeOp::Call(_, _, args) = op {
                            self.record(args, state);
                        }
                    }
                }
            }
            _ => {}
        }
    }
}

impl Visitor for CallCollector<'_> {
    fn visit_expr(&mut self, node: &Expr) {
        match node {
            Expr::Call { callee, args, .. } => {
                if let Some(name) = callee_name(callee) {
                    self.calls.push(call_info(name, args, self));
                }
                // Recurse into the callee's receiver (but not the callee name
                // itself) and the arguments.
                match callee.unparenthesized() {
                    Expr::Ident(_) => {}
                    Expr::Field { object, .. } => self.visit_expr(object),
                    other => self.visit_expr(other),
                }
                for a in &args.positional {
                    self.visit_expr(a);
                }
                for a in &args.named {
                    self.visit_expr(&a.value);
                }
            }
            Expr::Cascade {
                object, sections, ..
            } => {
                self.visit_expr(object);
                for section in sections {
                    for op in &section.ops {
                        match op {
                            CascadeOp::Call(ident, _, args) => {
                                self.calls.push(call_info(ident.name.clone(), args, self));
                                for a in &args.positional {
                                    self.visit_expr(a);
                                }
                                for a in &args.named {
                                    self.visit_expr(&a.value);
                                }
                            }
                            CascadeOp::Invoke(_, args) => {
                                for argument in &args.positional {
                                    self.visit_expr(argument);
                                }
                                for argument in &args.named {
                                    self.visit_expr(&argument.value);
                                }
                            }
                            CascadeOp::Index { index, .. } => {
                                self.visit_expr(index);
                            }
                            CascadeOp::Assign(target, _, value)
                            | CascadeOp::IndexAssign(target, _, value) => {
                                self.visit_expr(target);
                                self.visit_expr(value);
                            }
                            CascadeOp::Field { .. } => {}
                        }
                    }
                }
            }
            Expr::Ident(id) => {
                self.tear_offs.insert(id.name.clone());
            }
            _ => walk_expr(self, node),
        }
    }
}

// ── Body null-assignment scan ─────────────────────────────────────────────────

/// Parameter names that are assigned `null` somewhere in `body`.
fn body_null_assignments(body: Option<&FunctionBody>) -> HashSet<String> {
    let mut finder = NullAssignFinder {
        names: HashSet::new(),
    };
    if let Some(body) = body {
        walk_function_body(&mut finder, body);
    }
    finder.names
}

struct NullAssignFinder {
    names: HashSet<String>,
}

impl Visitor for NullAssignFinder {
    fn visit_expr(&mut self, node: &Expr) {
        if let Expr::Assign {
            target,
            op: AssignOp::Eq,
            value,
            ..
        } = node
            && let Expr::Ident(id) = target.unparenthesized()
            && is_null_expr(value)
        {
            self.names.insert(id.name.clone());
        }
        walk_expr(self, node);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use falcon_dart_parser::parse;

    fn count(source: &str) -> usize {
        let (program, errors) = parse(source);
        assert!(errors.is_empty(), "{errors:?}");
        UnnecessaryNullable
            .analyze_project(
                &[ProjectFile {
                    path: "test.dart".into(),
                    source: source.into(),
                    program,
                    has_parse_errors: false,
                }],
                &FalconConfig::default(),
                None,
            )
            .len()
    }

    #[test]
    fn invoked_callback_results_preserve_private_function_escape_evidence() {
        for invocation in ["holder..callback()(null)", "holder.callback()(null)"] {
            let source = format!(
                "void _take(int? value) {{}} \
                 class Holder {{ void Function(int?) callback() => _take; }} \
                 void use(Holder holder) {{ _take(1); {invocation}; }}"
            );
            assert_eq!(count(&source), 0, "{invocation}");
        }
        assert_eq!(
            count("void _take(int? value) {} void use() { _take(1); }"),
            1
        );
    }

    #[test]
    fn package_import_proves_nonnull_with_fallback_and_retained_indexes() {
        let root = std::env::temp_dir().join(format!(
            "falcon-nullable-package-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(root.join("lib")).unwrap();
        let manifest = root.join("pubspec.yaml");
        std::fs::write(&manifest, "name: app\n").unwrap();
        let config = FalconConfig::default();
        let mut registry = falcon_analyze::CrossFileRuleRegistry::new();
        registry.register(Box::new(UnnecessaryNullable));
        for (return_type, value, expected) in [("int", "1", 1), ("int?", "null", 0)] {
            let sources = vec![
                (root.join("lib/a.dart"), format!("{return_type} value() => {value};")),
                (
                    root.join("lib/b.dart"),
                    "import 'package:app/a.dart'; void _take(int? x) {} void f() { _take(value()); }".into(),
                ),
            ];
            for (path, source) in &sources {
                std::fs::write(path, source).unwrap();
            }
            let packages =
                crate::package_identities(sources.iter().map(|(path, _)| path.as_path()));
            let (diagnostics, mut files, indexes) =
                falcon_analyze::analyze_sequential_collecting_resolving(
                    &falcon_analyze::RuleRegistry::new(),
                    &sources,
                    &config,
                    true,
                    true,
                    falcon_analyze::Workspace {
                        packages: &packages,
                        environment: None,
                    },
                );
            assert!(diagnostics.is_empty(), "{diagnostics:?}");
            let (program, errors) = parse("");
            assert!(errors.is_empty());
            files.push(ProjectFile {
                path: manifest.clone(),
                source: "name: app\n".into(),
                program,
                has_parse_errors: false,
            });
            for indexes in [None, indexes.as_ref()] {
                let diagnostics = registry.run_all(&files, &config, indexes);
                assert_eq!(
                    diagnostics.len(),
                    expected,
                    "{return_type}: {diagnostics:?}"
                );
                if expected == 1 {
                    assert_eq!(diagnostics[0].rule, NAME);
                    assert_eq!(diagnostics[0].file_path, sources[1].0.to_string_lossy());
                }
            }
        }
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn known_return_proves_nonnull_but_local_shadow_does_not() {
        assert_eq!(
            count("int value() => 1; void _take(int? x) {} void f() { _take(value()); }"),
            1
        );
        assert_eq!(
            count(
                "int value() => 1; void _take(int? x) {} void f(int? Function() value) { _take(value()); }"
            ),
            0
        );
        assert_eq!(
            count(
                "int value() => 1; void _take(int? x) {} void f() { int? value() => null; _take(value()); }"
            ),
            0
        );
        assert_eq!(
            count("void _take(int? x) {} void f(int? x) { _take(x); }"),
            0
        );
        assert_eq!(
            count(
                "int value() => 1; void _take(int? x) {} class C { int? Function() get value => () => null; void f() { _take(value()); } }"
            ),
            0
        );
        assert_eq!(
            count(
                "int value() => 1; void _take(int? x) {} void f() { for (final value in <int? Function()>[() => null]) { _take(value()); } }"
            ),
            0
        );
    }
}
