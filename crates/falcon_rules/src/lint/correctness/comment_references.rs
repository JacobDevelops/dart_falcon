//! Flags doc-comment references `[name]` that resolve to nothing in scope.
//!
//! dartdoc links a bracketed name to the declaration it denotes, so a
//! misspelled or stale `[name]` renders as plain text and the link silently
//! breaks. The rule checks each `[name]`, `[Type.member]`, and
//! `[prefix.name]` reference in the doc comment of a declaration against the
//! names in scope there: the declaration's own parameters and type
//! parameters, the members of the enclosing class (including members
//! inherited from classes declared in the same file and those of `Object`),
//! the library's top-level declarations and import prefixes, and the names
//! the library imports, including `dart:core`. `[this]`, `[null]`, `[true]`,
//! and `[false]` are always flagged, since doc comments cannot reference
//! keywords. References defined as Markdown link references
//! (`/// [name]: https://...`), Markdown links (`[text](url)`,
//! `[text][ref]`), and code spans and code blocks are not references.
//! `[Type.new]` denotes `Type`'s unnamed constructor, declared or implicit.
//!
//! falcon only reports a name when it can see every declaration it could
//! denote: in files whose imports are all project libraries (or `dart:core`),
//! and, inside a class, only when its supertypes are declared in the same
//! file. Doc comments on library directives and local declarations,
//! `[new Name]` references, and members of declarations reached through a
//! prefix or another type's members (`[a.b.c]`) are not checked.
//!
//! Adopted from the official `comment_references` lint.

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use falcon_analyze::{AnalyzeContext, IdentityIndex, LibraryId, QualifiedName, Rule};
use falcon_dart_parser::lexer::Lexer;
use falcon_diagnostics::{Diagnostic, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::token::{Token, TokenKind};
use regex::Regex;

use crate::lint::semantic_scope::SemanticState;

/// Public top-level names of `dart:core` (including names removed in recent
/// SDKs, so older code is never reported).
const DART_CORE: &[&str] = &[
    "AbstractClassInstantiationError",
    "ArgumentError",
    "AssertionError",
    "BidirectionalIterator",
    "BigInt",
    "CastError",
    "Comparable",
    "Comparator",
    "ConcurrentModificationError",
    "CyclicInitializationError",
    "DateTime",
    "Deprecated",
    "Duration",
    "Enum",
    "EnumByName",
    "EnumName",
    "Error",
    "Exception",
    "Expando",
    "FallThroughError",
    "Finalizer",
    "FormatException",
    "Function",
    "Future",
    "IndexError",
    "IntegerDivisionByZeroException",
    "Invocation",
    "Iterable",
    "Iterator",
    "LateInitializationError",
    "List",
    "Map",
    "MapEntry",
    "Match",
    "Never",
    "NoSuchMethodError",
    "Null",
    "NullThrownError",
    "Object",
    "OutOfMemoryError",
    "Pattern",
    "Provisional",
    "Record",
    "RangeError",
    "RegExp",
    "RegExpMatch",
    "RuneIterator",
    "Runes",
    "Set",
    "Sink",
    "StackOverflowError",
    "StackTrace",
    "StateError",
    "Stopwatch",
    "Stream",
    "String",
    "StringBuffer",
    "StringSink",
    "Symbol",
    "Type",
    "TypeError",
    "UnimplementedError",
    "UnsupportedError",
    "Uri",
    "UriData",
    "WeakReference",
    "bool",
    "deprecated",
    "double",
    "dynamic",
    "identical",
    "identityHashCode",
    "int",
    "num",
    "override",
    "pragma",
    "print",
    "provisional",
    "proxy",
    "void",
];

/// Members every class inherits from `Object`.
const OBJECT_MEMBERS: &[&str] = &["hashCode", "noSuchMethod", "runtimeType", "toString"];

static REFERENCE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^[A-Za-z_$][A-Za-z0-9_$]*(?:\.[A-Za-z_$][A-Za-z0-9_$]*)*$")
        .expect("valid reference regex")
});

#[derive(Default)]
pub struct CommentReferences;

impl Rule for CommentReferences {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(state) = SemanticState::new(program, ctx) else {
            return Vec::new();
        };
        let library_scope = ctx.identities.and_then(|identities| {
            let library = identities.library(identities.file(ctx.file_path)?);
            Some(LibraryScope {
                identities,
                library,
                closed: identities.names_fully_indexed(library),
                implicit_core: !program
                    .imports
                    .iter()
                    .any(|import| import.uri.value == "dart:core" && import.as_name.is_some()),
                state,
            })
        });
        let types = type_scopes(program);
        let mut checker = Checker {
            ctx,
            program,
            types: &types,
            library: library_scope,
            diags: Vec::new(),
        };
        let contexts = doc_contexts(program, &types);
        let tokens = Lexer::new(ctx.source).tokenize();
        for block in doc_blocks(&tokens, ctx.source) {
            if let Some(context) = contexts
                .iter()
                .filter(|context| block.start <= context.start && context.start <= block.next_token)
                .max_by_key(|context| context.start)
            {
                checker.check_block(&block, context);
            }
        }
        checker.diags
    }
}

struct LibraryScope<'a> {
    identities: &'a IdentityIndex,
    library: LibraryId,
    closed: bool,
    implicit_core: bool,
    state: SemanticState<'a>,
}

/// Members and supertypes of a class-like declaration in this file.
#[derive(Default)]
struct TypeScope {
    members: HashSet<String>,
    type_params: Vec<String>,
    supertypes: Vec<String>,
    /// Members come from a type falcon does not see (an extension's on-type).
    open: bool,
    /// Whether `[Type.new]` denotes a constructor; `None` when unknown.
    unnamed_constructor: Option<bool>,
}

/// The names in scope in a declaration's doc comment.
struct DocContext {
    /// Where the declaration (or its first annotation) starts.
    start: usize,
    locals: Vec<String>,
    /// The class-like declaration whose members are in scope.
    owner: Option<String>,
    /// Names beyond these could be in scope (a typedef's function type).
    open: bool,
}

struct DocBlock {
    start: usize,
    /// Offset of the first token after the block.
    next_token: usize,
    tokens: Vec<(usize, usize)>,
}

fn declaration_start(span: &Span, annotations: &[Annotation]) -> usize {
    annotations.first().map_or(span.start, |annotation| {
        annotation.span.start.min(span.start)
    })
}

fn type_names(types: &[TypeParam]) -> Vec<String> {
    types.iter().map(|param| param.name.name.clone()).collect()
}

fn param_names(params: &FormalParamList) -> Vec<String> {
    params
        .params
        .iter()
        .map(|param| param.name.name.clone())
        .collect()
}

fn supertype_names<'a>(types: impl IntoIterator<Item = &'a DartType>) -> Vec<String> {
    types
        .into_iter()
        .map(|ty| match ty {
            DartType::Named(named) if named.segments.len() == 1 => named
                .segments
                .first()
                .map_or_else(String::new, |segment| segment.name.clone()),
            // Prefixed and structural supertypes are never declared here.
            DartType::Named(_)
            | DartType::Function(_)
            | DartType::Record(_)
            | DartType::Void { .. }
            | DartType::Dynamic { .. } => String::new(),
        })
        .collect()
}

fn member_names(members: &[ClassMember], scope: &mut TypeScope) {
    for member in members {
        match member {
            ClassMember::Field(field) => scope.members.extend(
                field
                    .declarators
                    .iter()
                    .map(|declarator| declarator.name.name.clone()),
            ),
            ClassMember::Constructor(constructor) => {
                if let Some(name) = &constructor.constructor_name {
                    scope.members.insert(name.name.clone());
                }
            }
            ClassMember::Method(method) => {
                scope.members.insert(method.name.name.clone());
            }
            ClassMember::Getter(getter) => {
                scope.members.insert(getter.name.name.clone());
            }
            ClassMember::Setter(setter) => {
                scope.members.insert(setter.name.name.clone());
            }
            ClassMember::Operator(_) => {}
            ClassMember::Error(_) => scope.open = true,
        }
    }
}

/// Whether `members` declare an unnamed constructor (`C()` or `C.new()`),
/// or, with `implicit`, declare no constructor so the default one exists.
/// `None` when an unparsed member could be one.
fn unnamed_constructor(members: &[ClassMember], implicit: bool) -> Option<bool> {
    let mut declared = false;
    let mut error = false;
    for member in members {
        match member {
            ClassMember::Constructor(constructor) => {
                if constructor
                    .constructor_name
                    .as_ref()
                    .is_none_or(|name| name.name == "new")
                {
                    return Some(true);
                }
                declared = true;
            }
            ClassMember::Error(_) => error = true,
            ClassMember::Field(_)
            | ClassMember::Method(_)
            | ClassMember::Getter(_)
            | ClassMember::Setter(_)
            | ClassMember::Operator(_) => {}
        }
    }
    (!error).then_some(implicit && !declared)
}

fn type_scopes(program: &Program) -> HashMap<String, TypeScope> {
    let mut scopes = HashMap::new();
    for declaration in &program.declarations {
        let mut scope = TypeScope::default();
        let name = match declaration {
            TopLevelDecl::Class(class) => {
                member_names(&class.members, &mut scope);
                scope.unnamed_constructor = unnamed_constructor(&class.members, true);
                scope.type_params = type_names(&class.type_params);
                scope.supertypes = supertype_names(
                    class
                        .extends
                        .iter()
                        .chain(&class.with_clause)
                        .chain(&class.implements),
                );
                &class.name.name
            }
            TopLevelDecl::MixinClass(class) => {
                member_names(&class.members, &mut scope);
                scope.unnamed_constructor = unnamed_constructor(&class.members, true);
                scope.type_params = type_names(&class.type_params);
                scope.supertypes = supertype_names(
                    class
                        .extends
                        .iter()
                        .chain(&class.with_clause)
                        .chain(&class.implements),
                );
                &class.name.name
            }
            TopLevelDecl::ClassTypeAlias(class) => {
                scope.type_params = type_names(&class.type_params);
                scope.supertypes = supertype_names(
                    std::iter::once(&class.superclass)
                        .chain(&class.with_clause)
                        .chain(&class.implements),
                );
                &class.name.name
            }
            TopLevelDecl::Mixin(mixin) => {
                member_names(&mixin.members, &mut scope);
                scope.type_params = type_names(&mixin.type_params);
                scope.supertypes = supertype_names(mixin.on_clause.iter().chain(&mixin.implements));
                &mixin.name.name
            }
            TopLevelDecl::Enum(enumeration) => {
                member_names(&enumeration.members, &mut scope);
                scope.unnamed_constructor = unnamed_constructor(&enumeration.members, true);
                scope.type_params = type_names(&enumeration.type_params);
                scope.supertypes = supertype_names(
                    enumeration
                        .with_clause
                        .iter()
                        .chain(&enumeration.implements),
                );
                scope.members.extend(
                    enumeration
                        .variants
                        .iter()
                        .map(|variant| variant.name.name.clone()),
                );
                scope
                    .members
                    .extend(["values", "index", "name"].map(String::from));
                &enumeration.name.name
            }
            TopLevelDecl::ExtensionType(extension) => {
                member_names(&extension.members, &mut scope);
                scope.type_params = type_names(&extension.type_params);
                scope.supertypes = supertype_names(&extension.implements);
                scope
                    .members
                    .insert(extension.representation.field_name.name.clone());
                if let Some(name) = &extension.representation.constructor_name {
                    scope.members.insert(name.name.clone());
                }
                scope.unnamed_constructor = if extension
                    .representation
                    .constructor_name
                    .as_ref()
                    .is_none_or(|name| name.name == "new")
                {
                    Some(true)
                } else {
                    unnamed_constructor(&extension.members, false)
                };
                &extension.name.name
            }
            TopLevelDecl::Extension(extension) => {
                let Some(name) = &extension.name else {
                    continue;
                };
                member_names(&extension.members, &mut scope);
                scope.type_params = type_names(&extension.type_params);
                scope.open = true;
                &name.name
            }
            TopLevelDecl::Function(_)
            | TopLevelDecl::Variable(_)
            | TopLevelDecl::TypeAlias(_)
            | TopLevelDecl::Error(_) => continue,
        };
        scopes.insert(name.clone(), scope);
    }
    scopes
}

/// Every member visible in `name`'s body, or `None` when some come from a
/// type outside this file.
fn all_members(
    name: &str,
    types: &HashMap<String, TypeScope>,
    visited: &mut HashSet<String>,
    out: &mut HashSet<String>,
) -> Option<()> {
    if !visited.insert(name.to_string()) {
        return Some(());
    }
    let scope = types.get(name)?;
    if scope.open {
        return None;
    }
    out.extend(scope.members.iter().cloned());
    for supertype in &scope.supertypes {
        all_members(supertype, types, visited, out)?;
    }
    Some(())
}

fn class_member_contexts(
    owner: &str,
    members: &[ClassMember],
    owner_types: &[String],
    contexts: &mut Vec<DocContext>,
) {
    for member in members {
        let (start, locals) = match member {
            ClassMember::Field(field) => (
                declaration_start(&field.span, &field.annotations),
                Vec::new(),
            ),
            ClassMember::Constructor(constructor) => (
                declaration_start(&constructor.span, &constructor.annotations),
                param_names(&constructor.params),
            ),
            ClassMember::Method(method) => {
                let mut locals = type_names(&method.type_params);
                locals.extend(param_names(&method.params));
                (declaration_start(&method.span, &method.annotations), locals)
            }
            ClassMember::Getter(getter) => (
                declaration_start(&getter.span, &getter.annotations),
                Vec::new(),
            ),
            ClassMember::Setter(setter) => (
                declaration_start(&setter.span, &setter.annotations),
                vec![setter.param.name.clone()],
            ),
            ClassMember::Operator(operator) => (
                declaration_start(&operator.span, &operator.annotations),
                param_names(&operator.params),
            ),
            ClassMember::Error(_) => continue,
        };
        let mut all = owner_types.to_vec();
        all.extend(locals);
        contexts.push(DocContext {
            start,
            locals: all,
            owner: Some(owner.to_string()),
            open: false,
        });
    }
}

fn doc_contexts(program: &Program, types: &HashMap<String, TypeScope>) -> Vec<DocContext> {
    let mut contexts = Vec::new();
    for declaration in &program.declarations {
        let owner_types = |name: &str| {
            types
                .get(name)
                .map(|scope| scope.type_params.clone())
                .unwrap_or_default()
        };
        let (start, locals, owner, open) = match declaration {
            TopLevelDecl::Class(class) => {
                let locals = owner_types(&class.name.name);
                class_member_contexts(&class.name.name, &class.members, &locals, &mut contexts);
                let start = declaration_start(&class.span, &class.annotations);
                (start, locals, Some(class.name.name.clone()), false)
            }
            TopLevelDecl::MixinClass(class) => {
                let locals = owner_types(&class.name.name);
                class_member_contexts(&class.name.name, &class.members, &locals, &mut contexts);
                let start = declaration_start(&class.span, &class.annotations);
                (start, locals, Some(class.name.name.clone()), false)
            }
            TopLevelDecl::ClassTypeAlias(class) => (
                declaration_start(&class.span, &class.annotations),
                owner_types(&class.name.name),
                Some(class.name.name.clone()),
                false,
            ),
            TopLevelDecl::Mixin(mixin) => {
                let locals = owner_types(&mixin.name.name);
                class_member_contexts(&mixin.name.name, &mixin.members, &locals, &mut contexts);
                let start = declaration_start(&mixin.span, &mixin.annotations);
                (start, locals, Some(mixin.name.name.clone()), false)
            }
            TopLevelDecl::Enum(enumeration) => {
                let name = &enumeration.name.name;
                let locals = owner_types(name);
                class_member_contexts(name, &enumeration.members, &locals, &mut contexts);
                for variant in &enumeration.variants {
                    contexts.push(DocContext {
                        start: declaration_start(&variant.span, &variant.annotations),
                        locals: locals.clone(),
                        owner: Some(name.clone()),
                        open: false,
                    });
                }
                let start = declaration_start(&enumeration.span, &enumeration.annotations);
                (start, locals, Some(name.clone()), false)
            }
            TopLevelDecl::ExtensionType(extension) => {
                let name = &extension.name.name;
                let locals = owner_types(name);
                class_member_contexts(name, &extension.members, &locals, &mut contexts);
                let start = declaration_start(&extension.span, &extension.annotations);
                (start, locals, Some(name.clone()), false)
            }
            TopLevelDecl::Extension(extension) => {
                let start = declaration_start(&extension.span, &extension.annotations);
                // The on-type's members are in scope and unseen here.
                (start, type_names(&extension.type_params), None, true)
            }
            TopLevelDecl::Function(function) => {
                let mut locals = type_names(&function.type_params);
                if let Some(params) = function.params() {
                    locals.extend(param_names(params));
                }
                (
                    declaration_start(&function.span, &function.annotations),
                    locals,
                    None,
                    false,
                )
            }
            TopLevelDecl::Variable(variable) => (
                declaration_start(&variable.span, &variable.annotations),
                Vec::new(),
                None,
                false,
            ),
            TopLevelDecl::TypeAlias(alias) => (
                declaration_start(&alias.span, &alias.annotations),
                type_names(&alias.type_params),
                None,
                true,
            ),
            TopLevelDecl::Error(_) => continue,
        };
        contexts.push(DocContext {
            start,
            locals,
            owner,
            open,
        });
    }
    contexts
}

/// Runs of doc-comment tokens separated only by whitespace.
fn doc_blocks(tokens: &[Token], source: &str) -> Vec<DocBlock> {
    let mut blocks: Vec<DocBlock> = Vec::new();
    let mut current: Option<DocBlock> = None;
    for token in tokens {
        if token.kind == TokenKind::DocComment {
            let block = current.get_or_insert_with(|| DocBlock {
                start: token.offset,
                next_token: source.len(),
                tokens: Vec::new(),
            });
            block.tokens.push((token.offset, token.offset + token.len));
        } else if !token.is_trivia() {
            if let Some(mut block) = current.take() {
                block.next_token = token.offset;
                blocks.push(block);
            }
        } else if matches!(token.kind, TokenKind::LineComment | TokenKind::BlockComment) {
            current = None;
        }
    }
    blocks
}

/// A `[reference]` in a doc comment: its text and source offset.
struct Reference<'s> {
    text: &'s str,
    offset: usize,
}

/// The references of a doc block and its Markdown link-reference definitions.
fn references<'s>(source: &'s str, block: &DocBlock) -> (Vec<Reference<'s>>, HashSet<&'s str>) {
    let mut found = Vec::new();
    let mut definitions = HashSet::new();
    let mut fenced = false;
    for &(start, end) in &block.tokens {
        let comment = &source[start..end];
        let mut line_start = start;
        for line in comment.split_inclusive('\n') {
            let offset = line_start;
            line_start += line.len();
            let trimmed = line.trim_start();
            let skipped = line.len() - trimmed.len();
            let marker = ["/**", "///", "*/", "*"]
                .iter()
                .find(|marker| trimmed.starts_with(*marker))
                .map_or(0, |marker| marker.len());
            let content = trimmed[marker..].trim_end_matches(['\n', '\r']);
            let content = content.strip_suffix("*/").unwrap_or(content);
            let content_offset = offset + skipped + marker;
            let text = content.strip_prefix(' ').unwrap_or(content);
            let text_offset = content_offset + (content.len() - text.len());
            if text.trim_start().starts_with("```") || text.trim_start().starts_with("~~~") {
                fenced = !fenced;
                continue;
            }
            if fenced || text.starts_with("    ") {
                continue;
            }
            scan_line(text, text_offset, &mut found, &mut definitions);
        }
    }
    (found, definitions)
}

fn scan_line<'s>(
    text: &'s str,
    offset: usize,
    found: &mut Vec<Reference<'s>>,
    definitions: &mut HashSet<&'s str>,
) {
    let bytes = text.as_bytes();
    let mut in_code = false;
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'`' => in_code = !in_code,
            b'[' if !in_code => {
                let Some(length) = text[index + 1..].find(']') else {
                    return;
                };
                let close = index + 1 + length;
                let inner = &text[index + 1..close];
                let next = bytes.get(close + 1).copied();
                if text[..index].trim().is_empty() && next == Some(b':') {
                    definitions.insert(inner);
                } else if next != Some(b'(') && next != Some(b'[') && !inner.contains('[') {
                    found.push(Reference {
                        text: inner,
                        offset: offset + index + 1,
                    });
                }
                index = close;
            }
            _ => {}
        }
        index += 1;
    }
}

struct Checker<'a> {
    ctx: &'a AnalyzeContext<'a>,
    program: &'a Program,
    types: &'a HashMap<String, TypeScope>,
    library: Option<LibraryScope<'a>>,
    diags: Vec<Diagnostic>,
}

/// Whether a name is in scope.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Found {
    Yes,
    No,
    Unknown,
}

impl Checker<'_> {
    fn check_block(&mut self, block: &DocBlock, context: &DocContext) {
        let source = self.ctx.source;
        let (found, definitions) = references(source, block);
        for reference in found {
            let text = reference.text;
            let report = if matches!(text, "this" | "null" | "true" | "false") {
                true
            } else if definitions.contains(text) || !REFERENCE.is_match(text) {
                false
            } else {
                self.unresolved(text, context)
            };
            if report {
                self.diags.push(self.ctx.diagnostic(
                    "comment-references",
                    format!("The referenced name '{text}' isn't visible in scope."),
                    DiagSpan {
                        start: reference.offset,
                        end: reference.offset + text.len(),
                    },
                ));
            }
        }
    }

    fn import_prefix(&self, name: &str) -> bool {
        self.program.imports.iter().any(|import| {
            import
                .as_name
                .as_ref()
                .is_some_and(|prefix| prefix.name == name)
        })
    }

    fn library_declares(&self, name: &str) -> bool {
        self.program
            .declarations
            .iter()
            .any(|declaration| match declaration {
                TopLevelDecl::Class(x) => x.name.name == name,
                TopLevelDecl::ClassTypeAlias(x) => x.name.name == name,
                TopLevelDecl::Mixin(x) => x.name.name == name,
                TopLevelDecl::MixinClass(x) => x.name.name == name,
                TopLevelDecl::Enum(x) => x.name.name == name,
                TopLevelDecl::Extension(x) => x.name.as_ref().is_some_and(|x| x.name == name),
                TopLevelDecl::ExtensionType(x) => x.name.name == name,
                TopLevelDecl::Function(x) => x.name.name == name,
                TopLevelDecl::Variable(x) => x.declarators.iter().any(|d| d.name.name == name),
                TopLevelDecl::TypeAlias(x) => x.name.name == name,
                TopLevelDecl::Error(_) => false,
            })
    }

    /// Members in scope in `owner`'s body, or `None` if falcon cannot see them all.
    fn owner_members(&self, owner: &str) -> Option<HashSet<String>> {
        let mut members: HashSet<String> =
            OBJECT_MEMBERS.iter().map(|name| name.to_string()).collect();
        all_members(owner, self.types, &mut HashSet::new(), &mut members)?;
        Some(members)
    }

    /// Whether the unqualified `name` is in scope in `context`.
    fn find(&self, name: &str, context: &DocContext) -> Found {
        if context.locals.iter().any(|local| local == name)
            || self.library_declares(name)
            || self.import_prefix(name)
        {
            return Found::Yes;
        }
        let mut found = Found::No;
        if let Some(owner) = &context.owner {
            match self.owner_members(owner) {
                Some(members) if members.contains(name) => return Found::Yes,
                Some(_) => {}
                None => found = Found::Unknown,
            }
        }
        if context.open {
            found = Found::Unknown;
        }
        let Some(library) = &self.library else {
            return Found::Unknown;
        };
        if library
            .identities
            .name_visible(library.library, QualifiedName::from(name))
            || (library.implicit_core && DART_CORE.contains(&name))
            || library
                .state
                .signatures
                .visible_extension_on_type(name, &library.state.model)
                .is_some()
        {
            return Found::Yes;
        }
        if !library.closed {
            return Found::Unknown;
        }
        found
    }

    fn unresolved(&self, text: &str, context: &DocContext) -> bool {
        let segments: Vec<&str> = text.split('.').collect();
        let Some((&first, rest)) = segments.split_first() else {
            return false;
        };
        match self.find(first, context) {
            Found::No => return true,
            Found::Unknown => return false,
            Found::Yes => {}
        }
        let [second] = rest else {
            return false;
        };
        if context.locals.iter().any(|local| local == first) {
            return false;
        }
        if self.import_prefix(first) {
            return self.library.as_ref().is_some_and(|library| {
                library.closed
                    && !library
                        .identities
                        .name_visible(library.library, QualifiedName::prefixed(first, second))
            });
        }
        if let Some(scope) = self.types.get(first)
            && self.library.as_ref().is_some_and(|library| library.closed)
        {
            if *second == "new" {
                return scope.unnamed_constructor == Some(false);
            }
            if let Some(members) = self.owner_members(first) {
                return !members.contains(*second);
            }
        }
        false
    }
}
