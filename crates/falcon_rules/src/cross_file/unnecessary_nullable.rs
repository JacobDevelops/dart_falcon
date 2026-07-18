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

use falcon_analyze::resolve::ProgramSource;
use falcon_analyze::{LocalTypes, ProjectFile, ProjectIndex, CrossFileRule, StaticType};
use falcon_config::FalconConfig;
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr};

pub struct UnnecessaryNullable;

const NAME: &str = "unnecessary-nullable";

impl CrossFileRule for UnnecessaryNullable {
    fn name(&self) -> &'static str {
        NAME
    }

    fn analyze_project(&self, files: &[ProjectFile], _config: &FalconConfig) -> Vec<Diagnostic> {
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

        // Cross-file return-type index, so a call/field argument whose declared
        // return type is a known non-nullable type can be recognized as non-null
        // (strengthening the literal-`null` matching below).
        let sources: Vec<ProgramSource> = files
            .iter()
            .map(|f| ProgramSource {
                program: &f.program,
                has_parse_errors: f.has_parse_errors,
            })
            .collect();
        let index = ProjectIndex::from_project_files(&sources);

        // Gather every call site (and bare tear-off reference) across all files.
        let mut collector = CallCollector::new(&index);
        for f in files {
            collector.visit_program(&f.program);
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
                if collector.tear_offs.contains(&target.name) {
                    continue;
                }
                let calls: Vec<&CallInfo> = collector
                    .calls
                    .iter()
                    .filter(|c| c.name == target.name)
                    .collect();
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
                        DiagSpan {
                            start: param.span.start,
                            end: param.span.end,
                        },
                    ));
                }
            }
        }
        diags
    }
}

/// Where a nullable parameter sits in a call's argument list.
enum ParamSlot {
    /// Positional index within `positional ++ optional_positional`; `optional`
    /// marks the optional-positional tail (which a call may legally omit).
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
                if is_private(&f.name.name) && !f.is_getter && !f.is_setter =>
            {
                push_target(&mut out, &f.name.name, &f.params, f.body.as_ref());
            }
            TopLevelDecl::Class(c) => collect_method_targets(&mut out, &c.members),
            TopLevelDecl::Mixin(m) => collect_method_targets(&mut out, &m.members),
            TopLevelDecl::MixinClass(m) => collect_method_targets(&mut out, &m.members),
            TopLevelDecl::Enum(e) => collect_method_targets(&mut out, &e.members),
            TopLevelDecl::Extension(e) => collect_method_targets(&mut out, &e.members),
            TopLevelDecl::ExtensionType(e) => collect_method_targets(&mut out, &e.members),
            _ => {}
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
    let positional_len = params.positional.len();
    for (i, p) in params
        .positional
        .iter()
        .chain(&params.optional_positional)
        .enumerate()
    {
        if let Some(np) = make_param(
            p,
            ParamSlot::Positional {
                index: i,
                optional: i >= positional_len,
            },
        ) {
            out.push(np);
        }
    }
    for p in &params.named {
        if let Some(np) = make_param(
            p,
            ParamSlot::Named {
                optional: !p.is_required,
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
    if p.is_field || p.is_super || p.function_params.is_some() {
        return None;
    }
    let ty = p.param_type.as_ref()?;
    if !ty.is_nullable() {
        return None;
    }
    let default_is_null = match &p.default_value {
        None => true,
        Some(Expr::NullLit { .. }) => true,
        Some(_) => false,
    };
    Some(NullableParam {
        name: p.name.name.clone(),
        span: p.name.span.clone(),
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
            TopLevelDecl::Function(f) if !f.is_getter && !f.is_setter => {
                out.push(f.name.name.clone())
            }
            TopLevelDecl::Class(c) => method_names(&mut out, &c.members),
            TopLevelDecl::Mixin(m) => method_names(&mut out, &m.members),
            TopLevelDecl::MixinClass(m) => method_names(&mut out, &m.members),
            TopLevelDecl::Enum(e) => method_names(&mut out, &e.members),
            TopLevelDecl::Extension(e) => method_names(&mut out, &e.members),
            TopLevelDecl::ExtensionType(e) => method_names(&mut out, &e.members),
            _ => {}
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
    matches!(e, Expr::NullLit { .. })
}

/// Whether an argument expression *might* be null. Literal `null` obviously is;
/// beyond that an argument is proven non-null only when [`LocalTypes::of_expr`]
/// infers a non-nullable type (literals, `new`, arithmetic, …) or the project
/// index resolves the callee/getter to a known non-nullable return type.
/// Everything else stays "maybe null" so the rule never flags a param that could
/// legitimately receive null.
fn arg_maybe_null(arg: &Expr, lt: &LocalTypes, index: &ProjectIndex) -> bool {
    if is_null_expr(arg) {
        return true;
    }
    if !lt.of_expr(arg).is_nullable() {
        return false;
    }
    if let Some(name) = arg_return_name(arg) {
        let rt = index.return_type(&name);
        if rt != StaticType::Unknown && !rt.is_nullable() {
            return false;
        }
    }
    true
}

/// The name whose declared return type describes `arg`'s value: the callee of a
/// call, or the accessed getter/field.
fn arg_return_name(arg: &Expr) -> Option<String> {
    match arg {
        Expr::Call { callee, .. } => callee_name(callee),
        Expr::Field { field, .. } => Some(field.name.clone()),
        _ => None,
    }
}

fn call_info(name: String, args: &ArgList, lt: &LocalTypes, index: &ProjectIndex) -> CallInfo {
    CallInfo {
        name,
        positional_maybe_null: args
            .positional
            .iter()
            .map(|a| arg_maybe_null(a, lt, index))
            .collect(),
        named_maybe_null: args
            .named
            .iter()
            .map(|a| (a.name.name.clone(), arg_maybe_null(&a.value, lt, index)))
            .collect(),
    }
}

fn callee_name(callee: &Expr) -> Option<String> {
    match callee {
        Expr::Ident(id) => Some(id.name.clone()),
        Expr::Field { field, .. } => Some(field.name.clone()),
        _ => None,
    }
}

struct CallCollector<'a> {
    calls: Vec<CallInfo>,
    /// Names used as bare values (tear-offs), not as a call callee.
    tear_offs: HashSet<String>,
    index: &'a ProjectIndex,
    /// A file-local scope tracker with no bindings: it resolves the structurally
    /// non-null argument forms (literals, `new`, arithmetic) without full scope
    /// tracking, which is all the null-flow heuristic needs.
    lt: LocalTypes,
}

impl<'a> CallCollector<'a> {
    fn new(index: &'a ProjectIndex) -> Self {
        Self {
            calls: Vec::new(),
            tear_offs: HashSet::new(),
            index,
            lt: LocalTypes::new(),
        }
    }
}

impl Visitor for CallCollector<'_> {
    fn visit_expr(&mut self, node: &Expr) {
        match node {
            Expr::Call { callee, args, .. } => {
                if let Some(name) = callee_name(callee) {
                    self.calls.push(call_info(name, args, &self.lt, self.index));
                }
                // Recurse into the callee's receiver (but not the callee name
                // itself) and the arguments.
                match &**callee {
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
                                self.calls.push(call_info(
                                    ident.name.clone(),
                                    args,
                                    &self.lt,
                                    self.index,
                                ));
                                for a in &args.positional {
                                    self.visit_expr(a);
                                }
                                for a in &args.named {
                                    self.visit_expr(&a.value);
                                }
                            }
                            CascadeOp::Index(index, _) => {
                                self.visit_expr(index);
                            }
                            CascadeOp::Assign(target, _, value) => {
                                self.visit_expr(target);
                                self.visit_expr(value);
                            }
                            CascadeOp::Field(..) => {}
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
        match body {
            FunctionBody::Block(b) => {
                for s in &b.stmts {
                    finder.visit_stmt(s);
                }
            }
            FunctionBody::Arrow(e, _) => finder.visit_expr(e),
            FunctionBody::Native(_, _) => {}
        }
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
            && let Expr::Ident(id) = &**target
            && is_null_expr(value)
        {
            self.names.insert(id.name.clone());
        }
        walk_expr(self, node);
    }
}
