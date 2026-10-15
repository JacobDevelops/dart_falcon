//! Doc-comment scanning and resolution shared by the rules that read
//! `[reference]`s.
//!
//! Scanning finds blocks of consecutive doc-comment tokens and the bracketed
//! references in them as the analyzer's doc-comment parser does, skipping
//! code spans, fenced and indented code blocks, Markdown links and
//! link-reference definitions. Resolution follows the analyzer's
//! `CommentReferenceResolver`: a simple name is looked up lexically (the
//! declaration's parameters and type parameters, the members the enclosing
//! type or extension declares, the library scope, then the `@docImport`s) and
//! then as a member of the enclosing type; `Type.name` finds an instance
//! member (inherited ones too) or a declared static member or constructor;
//! `prefix.Type.name` only declared members.
//!
//! Primary constructor headers provide constructor parameter scopes; their
//! declaring parameters also contribute instance members. A `this` body is
//! only the implementation, never a second documentation declaration.
//! Resolved members and parameters borrow their names from the reference text;
//! typed selectors preserve access, static dispatch and operator arity without
//! allocating encoded annotation keys.

use std::collections::{HashMap, HashSet};

use falcon_analyze::resolve::TypeKind;
use falcon_analyze::{
    DeclarationIdentity, DocName, MemberFacts, MemberKind, MemberSelector, MemberSelectorKind,
    NameIdentity, ResolvedType, SemanticModel, SignatureIndex, SourceSelector, TypeParameterScope,
};
use falcon_syntax::ast::*;
use falcon_syntax::token::{Token, TokenKind};

/// A run of doc-comment tokens separated only by whitespace.
pub(crate) struct DocBlock {
    pub(crate) start: usize,
    /// Offset of the first token after the block.
    pub(crate) next_token: usize,
    /// `(start, end)` of each doc-comment token.
    pub(crate) tokens: Vec<(usize, usize)>,
}

/// The doc-comment tokens before each declaration, as the analyzer collects
/// a documentation comment: every `///` token since the previous code token
/// (plain comments between them are skipped), or a `/**` comment and what
/// follows it.
pub(crate) fn doc_blocks(tokens: &[Token], source: &str) -> Vec<DocBlock> {
    let mut blocks: Vec<DocBlock> = Vec::new();
    let mut current: Option<DocBlock> = None;
    for token in tokens {
        if token.kind == TokenKind::DocComment {
            if source[token.offset..].starts_with("/**") {
                current = None;
            }
            let block = current.get_or_insert_with(|| DocBlock {
                start: token.offset,
                next_token: source.len(),
                tokens: Vec::new(),
            });
            block.tokens.push((token.offset, token.offset + token.len));
        } else if !token.is_trivia()
            && let Some(mut block) = current.take()
        {
            block.next_token = token.offset;
            blocks.push(block);
        }
    }
    blocks
}

/// A `[reference]` in a doc comment: its text and source offset.
pub(crate) struct Reference<'s> {
    pub(crate) text: &'s str,
    pub(crate) offset: usize,
}

/// The lines of a doc-comment token as the analyzer's doc-comment parser
/// reads them: a `///` line without its slashes; a `/** */` comment's first
/// line whole, later lines without leading whitespace and a `* ` (or lone
/// `*`) marker. Each line comes with its source offset.
fn comment_lines(lexeme: &str, start: usize) -> Vec<(usize, &str)> {
    if let Some(content) = lexeme.strip_prefix("///") {
        return vec![(start + 3, content)];
    }
    let mut lines = Vec::new();
    let mut offset = 0;
    for (index, line) in lexeme.split('\n').enumerate() {
        let at = offset;
        offset += line.len() + 1;
        if index == 0 {
            lines.push((start, line));
            continue;
        }
        let content = line.trim_start_matches([' ', '\t', '\r']);
        let mut at = at + line.len() - content.len();
        let content = if let Some(rest) = content.strip_prefix("* ") {
            at += 2;
            rest
        } else if content == "*" {
            at += 1;
            ""
        } else {
            content
        };
        lines.push((start + at, content));
    }
    lines
}

/// Dartdoc directives whose lines carry no references.
const DOC_DIRECTIVES: &[&str] = &[
    "animation",
    "canonicalFor",
    "category",
    "end-inject-html",
    "end-tool",
    "endtemplate",
    "example",
    "inject-html",
    "macro",
    "subCategory",
    "template",
    "tool",
    "youtube",
];

fn leading_whitespace(content: &str) -> usize {
    content.len() - content.trim_start_matches([' ', '\t', '\r', '\n']).len()
}

/// The references of a doc block and the Markdown link-reference
/// definitions the `comment_references` lint collects (`/// [name]: ...`).
///
/// Mirrors the analyzer's `DocCommentBuilder`: indented code blocks (four
/// columns of whitespace after a blank line), backtick-fenced code blocks,
/// dartdoc directive, `@docImport` and `@nodoc` lines carry no references.
pub(crate) fn references<'s>(
    source: &'s str,
    block: &DocBlock,
) -> (Vec<Reference<'s>>, HashSet<&'s str>) {
    let mut found = Vec::new();
    let mut definitions = HashSet::new();
    let mut previous_empty = true;
    let mut indented = false;
    let mut fence: Option<usize> = None;
    for &(start, end) in &block.tokens {
        let lexeme = &source[start..end];
        if let Some(left) = lexeme.find('[')
            && let Some(length) = lexeme[left..].find(']')
            && lexeme[..left].trim_end().starts_with("///")
            && lexeme[..left].trim_end().trim_start_matches('/').is_empty()
            && lexeme[left + length + 1..].starts_with(':')
        {
            definitions.insert(&lexeme[left + 1..left + length]);
        }
        for (offset, content) in comment_lines(lexeme, start) {
            let whitespace = leading_whitespace(content);
            let rest = &content[whitespace..];
            if indented && whitespace >= 4 {
                continue;
            }
            indented = false;
            if previous_empty && whitespace >= 4 {
                indented = true;
                continue;
            }
            previous_empty = false;
            if let Some(ticks) = fence {
                if ticks == 3 && rest.starts_with("```") {
                    fence = None;
                }
                continue;
            }
            if rest.starts_with("```") {
                fence = Some(rest.len() - rest.trim_start_matches('`').len());
                continue;
            }
            let directive = rest.strip_prefix("{@").is_some_and(|directive| {
                let name = directive
                    .split(|c: char| c.is_whitespace() || c == '}')
                    .next()
                    .unwrap_or_default();
                DOC_DIRECTIVES.contains(&name)
            });
            let nodoc = rest
                .strip_prefix("@nodoc")
                .is_some_and(|after| after.is_empty() || after.starts_with(' '));
            if directive || rest.starts_with("@docImport ") || nodoc {
                continue;
            }
            scan_line(content, offset, &mut found);
            previous_empty = content.is_empty();
        }
    }
    (found, definitions)
}

/// `[this]`, `[null]`, `[true]` and `[false]` anywhere in the block's tokens
/// (code spans included), as the `comment_references` lint scans each token
/// from one `[` to the next `]`; with their offsets.
pub(crate) fn keyword_references<'s>(source: &'s str, block: &DocBlock) -> Vec<(usize, &'s str)> {
    let mut found = Vec::new();
    for &(start, end) in &block.tokens {
        let lexeme = &source[start..end];
        let mut from = 0;
        while let Some(left) = lexeme[from..].find('[').map(|left| from + left)
            && let Some(right) = lexeme[left..].find(']').map(|right| left + right)
        {
            let inner = &lexeme[left + 1..right];
            if matches!(inner, "this" | "null" | "true" | "false") {
                found.push((start + left + 1, inner));
            }
            from = right;
        }
    }
    found
}

/// The analyzer's `_parseReferences` over one line: a bracketed text followed
/// by `(`, by optional whitespace and `[`, or (with only whitespace before it)
/// by `:` is Markdown link text, not a reference; a backtick skips to the
/// next one and `[:` to the next `:]`.
fn scan_line<'s>(text: &'s str, offset: usize, found: &mut Vec<Reference<'s>>) {
    let bytes = text.as_bytes();
    let mut index = 0;
    let mut only_whitespace = true;
    while index < bytes.len() {
        match bytes[index] {
            b'[' => {
                if bytes.get(index + 1) == Some(&b':') {
                    match text[index + 2..].find(":]") {
                        Some(end) => index += 2 + end + 1,
                        None => return,
                    }
                } else {
                    let Some(length) = text[index + 1..].find(']') else {
                        return;
                    };
                    let close = index + 1 + length;
                    let after = &text[close + 1..];
                    let link = after.starts_with('(')
                        || (only_whitespace && after.starts_with(':'))
                        || after
                            .trim_start_matches([' ', '\t', '\r', '\n'])
                            .starts_with('[');
                    if !link {
                        found.push(Reference {
                            text: &text[index + 1..close],
                            offset: offset + index + 1,
                        });
                    }
                    index = close;
                }
                only_whitespace = false;
            }
            b'`' => {
                if let Some(end) = text[index + 1..].find('`') {
                    index += end + 1;
                }
                only_whitespace = false;
            }
            byte if !byte.is_ascii_whitespace() => only_whitespace = false,
            _ => {}
        }
        index += 1;
    }
}

/// One name of a parsed reference and its source offset.
pub(crate) struct RefName<'s> {
    pub(crate) text: &'s str,
    pub(crate) offset: usize,
}

/// A reference as the analyzer's doc-comment parser reads it: an optional
/// `new`, then one to three dot-separated names, the last possibly an
/// (optionally `operator`-prefixed) user-definable operator.
pub(crate) struct ParsedReference<'s> {
    pub(crate) new_keyword: bool,
    pub(crate) names: Vec<RefName<'s>>,
}

/// Dart's reserved words, which cannot name a referenced declaration.
const RESERVED: &[&str] = &[
    "assert", "break", "case", "catch", "class", "const", "continue", "default", "do", "else",
    "enum", "extends", "false", "final", "finally", "for", "if", "in", "is", "new", "null",
    "rethrow", "return", "super", "switch", "this", "throw", "true", "try", "var", "void", "while",
    "with",
];

/// The user-definable operators, longest first.
const OPERATORS: &[&str] = &[
    ">>>", "[]=", "==", "<=", ">=", "<<", ">>", "~/", "[]", "<", ">", "-", "+", "/", "*", "%", "|",
    "^", "&", "~",
];

enum RefToken<'s> {
    Word(&'s str, usize),
    Dot,
    Operator(&'s str, usize),
    Other,
}

impl<'s> RefToken<'s> {
    fn identifier(&self, allow_new: bool) -> Option<RefName<'s>> {
        match *self {
            RefToken::Word(text, offset)
                if !RESERVED.contains(&text) || (allow_new && text == "new") =>
            {
                Some(RefName { text, offset })
            }
            _ => None,
        }
    }
}

fn ref_tokens(text: &str, offset: usize) -> Vec<RefToken<'_>> {
    let mut tokens = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find(|c: char| !c.is_whitespace()) {
        rest = &rest[start..];
        let at = offset + text.len() - rest.len();
        let word = rest
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '$'))
            .unwrap_or(rest.len());
        let token = if word > 0 && !rest.as_bytes()[0].is_ascii_digit() {
            (RefToken::Word(&rest[..word], at), word)
        } else if rest.starts_with('.') && !rest[1..].starts_with('.') {
            (RefToken::Dot, 1)
        } else if let Some(operator) = OPERATORS.iter().find(|op| rest.starts_with(**op)) {
            (
                RefToken::Operator(&rest[..operator.len()], at),
                operator.len(),
            )
        } else {
            (RefToken::Other, rest.len())
        };
        tokens.push(token.0);
        rest = &rest[token.1..];
    }
    tokens
}

impl<'s> Reference<'s> {
    /// The reference as the analyzer parses it; `None` when it is not one.
    pub(crate) fn parse(&self) -> Option<ParsedReference<'s>> {
        let tokens = ref_tokens(self.text, self.offset);
        let mut index = 0;
        let new_keyword = matches!(tokens.first(), Some(RefToken::Word("new", _)));
        if new_keyword {
            index = 1;
        }
        let mut names = Vec::new();
        let dot = |at: usize| matches!(tokens.get(at), Some(RefToken::Dot));
        if let Some(name) = tokens.get(index).and_then(|token| token.identifier(false))
            && dot(index + 1)
        {
            names.push(name);
            index += 2;
            if let Some(name) = tokens.get(index).and_then(|token| token.identifier(false))
                && dot(index + 1)
            {
                names.push(name);
                index += 2;
            }
        }
        let after_dot = !names.is_empty();
        if matches!(tokens.get(index), Some(RefToken::Word("operator", _)))
            && let Some(RefToken::Operator(text, offset)) = tokens.get(index + 1)
        {
            index += 1;
            names.push(RefName {
                text,
                offset: *offset,
            });
        } else {
            let token = tokens.get(index)?;
            names.push(match token {
                RefToken::Operator(text, offset) => RefName {
                    text,
                    offset: *offset,
                },
                _ => token.identifier(after_dot)?,
            });
        }
        (index + 1 == tokens.len()).then_some(ParsedReference { new_keyword, names })
    }
}

/// Public top-level names of `dart:core` (including names removed in recent
/// SDKs), which are in scope without loaded SDK sources.
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

/// Members every type inherits from `Object`, known without SDK sources;
/// enums also have `index` and `name`.
const OBJECT_MEMBERS: &[&str] = &["==", "hashCode", "noSuchMethod", "runtimeType", "toString"];

/// The type or extension whose members a doc comment sees.
pub(crate) enum Enclosing {
    None,
    /// A class, mixin, enum or extension type; `identity` is `None` when its
    /// name does not resolve uniquely.
    Type {
        identity: Option<DeclarationIdentity>,
        /// Declared members (no constructors), and whether each is static.
        members: HashMap<String, bool>,
    },
    Extension {
        on_type: ResolvedType,
        members: HashMap<String, bool>,
    },
}

/// The names a declaration's doc comment sees.
pub(crate) struct DocScope {
    /// Where the declaration (or its first annotation) starts.
    pub(crate) start: usize,
    /// The declaration.
    pub(crate) span: Span,
    /// Parameters and type parameters in scope.
    locals: Vec<String>,
    enclosing: std::rc::Rc<Enclosing>,
    /// The top-level function this doc comment documents, with its
    /// parameters.
    function: Option<(DeclarationIdentity, Vec<String>)>,
}

/// What a referenced name denotes.
pub(crate) enum DocElement<'s> {
    /// A parameter or type parameter.
    Local,
    Prefix(&'s str),
    /// A top-level declaration (see [`DocName::Declaration`]).
    Declaration {
        identity: Option<DeclarationIdentity>,
        kind: NameIdentity,
    },
    /// A named extension.
    Extension(&'s str),
    /// A member of `owner`, optionally selecting one of its parameters.
    Member {
        owner: DeclarationIdentity,
        selector: SourceSelector<'s>,
    },
    /// A member falcon does not identify (an extension's or one reached
    /// through an unidentified type).
    Opaque,
}

pub(crate) enum DocLookup<'s> {
    Found(DocElement<'s>),
    Absent,
    Unknown,
}

impl<'s> DocLookup<'s> {
    fn absent_or(self, other: impl FnOnce() -> Self) -> Self {
        match self {
            DocLookup::Absent => other(),
            found => found,
        }
    }
}

/// Every doc-commentable declaration of a file and what its doc comment sees.
pub(crate) struct DocScopes {
    scopes: Vec<DocScope>,
    /// Declared members of the file's named extensions.
    extensions: HashMap<String, HashMap<String, bool>>,
}

fn declaration_start(span: &Span, annotations: &[Annotation]) -> usize {
    annotations.first().map_or(span.start, |annotation| {
        annotation.span.start.min(span.start)
    })
}

fn type_names(types: &[TypeParam]) -> impl Iterator<Item = String> + '_ {
    types.iter().map(|param| param.name.name.clone())
}

fn param_names(params: &FormalParamList) -> impl Iterator<Item = String> + '_ {
    params.params.iter().map(|param| param.name.name.clone())
}

fn declared_members(members: EffectiveMembers<'_>) -> HashMap<String, bool> {
    let mut names = HashMap::new();
    for member in members.declared {
        match member {
            ClassMember::Field(field) => {
                for declarator in &field.declarators {
                    names.insert(declarator.name.name.clone(), field.is_static);
                }
            }
            ClassMember::Method(method) => {
                names.insert(method.name.name.clone(), method.is_static);
            }
            ClassMember::Getter(getter) => {
                names.insert(getter.name.name.clone(), getter.is_static);
            }
            ClassMember::Setter(setter) => {
                names.insert(setter.name.name.clone(), setter.is_static);
            }
            ClassMember::Operator(operator) => {
                names.insert(operator.op.clone(), false);
            }
            ClassMember::Constructor(_)
            | ClassMember::PrimaryConstructorBody(_)
            | ClassMember::Error(_) => {}
        }
    }
    names.extend(
        members
            .declaring_params()
            .map(|param| (param.name.name.clone(), false)),
    );
    names
}

impl DocScopes {
    pub(crate) fn new(program: &Program, model: &SemanticModel<'_>, source: &str) -> Self {
        let mut scopes = Self {
            scopes: Vec::new(),
            extensions: HashMap::new(),
        };
        let none = std::rc::Rc::new(Enclosing::None);
        let directives = program
            .library_directive
            .iter()
            .map(|node| (node.span, node.annotations.as_slice()))
            .chain(
                program
                    .imports
                    .iter()
                    .map(|node| (node.span, node.annotations.as_slice())),
            )
            .chain(
                program
                    .exports
                    .iter()
                    .map(|node| (node.span, node.annotations.as_slice())),
            )
            .chain(
                program
                    .part_directives
                    .iter()
                    .map(|node| (node.span, node.annotations.as_slice())),
            );
        for (span, annotations) in directives {
            scopes.push(span, annotations, Vec::new(), &none);
        }
        for declaration in &program.declarations {
            scopes.declaration(declaration, model, source, &none);
        }
        scopes
    }

    fn push(
        &mut self,
        span: Span,
        annotations: &[Annotation],
        locals: Vec<String>,
        enclosing: &std::rc::Rc<Enclosing>,
    ) {
        self.scopes.push(DocScope {
            start: declaration_start(&span, annotations),
            span,
            locals,
            enclosing: enclosing.clone(),
            function: None,
        });
    }

    fn declaration(
        &mut self,
        declaration: &TopLevelDecl,
        model: &SemanticModel<'_>,
        source: &str,
        none: &std::rc::Rc<Enclosing>,
    ) {
        let span = *declaration.span();
        let (name, annotations, type_params, members) = match declaration {
            TopLevelDecl::Class(n) => {
                (&n.name, &n.annotations, &n.type_params, n.element_members())
            }
            TopLevelDecl::MixinClass(n) => {
                (&n.name, &n.annotations, &n.type_params, n.element_members())
            }
            TopLevelDecl::Mixin(n) => (
                &n.name,
                &n.annotations,
                &n.type_params,
                EffectiveMembers::declared(&n.members),
            ),
            TopLevelDecl::Enum(n) => (&n.name, &n.annotations, &n.type_params, n.element_members()),
            TopLevelDecl::ExtensionType(n) => {
                (&n.name, &n.annotations, &n.type_params, n.element_members())
            }
            TopLevelDecl::Extension(extension) => {
                let mut parameters = TypeParameterScope::default();
                parameters.push(&extension.type_params, model);
                let members = declared_members(EffectiveMembers::declared(&extension.members));
                if let Some(name) = &extension.name {
                    self.extensions.insert(name.name.clone(), members.clone());
                }
                let enclosing = std::rc::Rc::new(Enclosing::Extension {
                    on_type: model.resolve_type_in(&extension.on_type, &parameters),
                    members,
                });
                let locals: Vec<_> = type_names(&extension.type_params).collect();
                self.push(span, &extension.annotations, locals.clone(), &enclosing);
                self.members(&extension.members, &locals, &enclosing);
                return;
            }
            TopLevelDecl::ClassTypeAlias(n) => {
                let enclosing = std::rc::Rc::new(Enclosing::Type {
                    identity: model.resolve_name(&n.name.name),
                    members: HashMap::new(),
                });
                self.push(
                    span,
                    &n.annotations,
                    type_names(&n.type_params).collect(),
                    &enclosing,
                );
                return;
            }
            TopLevelDecl::Function(function) => {
                let mut locals: Vec<_> = type_names(&function.type_params).collect();
                locals.extend(function.params().into_iter().flat_map(param_names));
                if let Some(params) = function.params() {
                    self.params(params, &locals, none);
                }
                self.push(span, &function.annotations, locals, none);
                if let (Some(identity), Some(params)) =
                    (model.resolve_value(&function.name.name), function.params())
                    && let Some(scope) = self.scopes.last_mut()
                {
                    scope.function = Some((identity, param_names(params).collect()));
                }
                return;
            }
            TopLevelDecl::Variable(variable) => {
                self.push(span, &variable.annotations, Vec::new(), none);
                return;
            }
            TopLevelDecl::TypeAlias(alias) => {
                // A function-type alias also scopes the function type's own
                // type parameters and named parameters.
                let mut locals: Vec<_> = type_names(&alias.type_params).collect();
                if let DartType::Function(function) = &alias.aliased {
                    locals.extend(type_names(&function.type_params));
                    locals.extend(
                        function
                            .params
                            .iter()
                            .filter_map(|param| param.name.as_ref())
                            .map(|name| name.name.clone()),
                    );
                }
                if let DartType::Function(function) = &alias.aliased {
                    for param in &function.params {
                        // A `required` modifier precedes the parameter type.
                        let ty = param.param_type.span();
                        let before = source[..ty.start].trim_end();
                        let start = if param.is_required && before.ends_with("required") {
                            before.len() - "required".len()
                        } else {
                            ty.start
                        };
                        let end = param.name.as_ref().map_or(ty.end, |name| name.span.end);
                        self.push(Span::new(start, end), &[], locals.clone(), none);
                    }
                }
                self.push(span, &alias.annotations, locals, none);
                return;
            }
            TopLevelDecl::Error(_) => return,
        };
        let mut declared = declared_members(members);
        if let TopLevelDecl::Enum(enumeration) = declaration {
            declared.extend(
                enumeration
                    .variants
                    .iter()
                    .map(|variant| (variant.name.name.clone(), true)),
            );
            declared.insert("values".to_owned(), true);
        }
        let enclosing = std::rc::Rc::new(Enclosing::Type {
            identity: model.resolve_name(&name.name),
            members: declared,
        });
        let locals: Vec<_> = type_names(type_params).collect();
        let mut header_locals = locals.clone();
        if let Some(primary) = members.primary {
            header_locals.extend(param_names(&primary.params));
            self.params(&primary.params, &header_locals, &enclosing);
            self.push(primary.span, &[], header_locals.clone(), &enclosing);
        }
        self.push(span, annotations, header_locals, &enclosing);
        if let TopLevelDecl::Enum(enumeration) = declaration {
            for variant in &enumeration.variants {
                self.push(
                    variant.span,
                    &variant.annotations,
                    locals.clone(),
                    &enclosing,
                );
            }
        }
        self.members(members.declared, &locals, &enclosing);
    }

    fn members(
        &mut self,
        members: &[ClassMember],
        owner_locals: &[String],
        enclosing: &std::rc::Rc<Enclosing>,
    ) {
        for member in members {
            let mut locals = owner_locals.to_vec();
            let (span, annotations, params) = match member {
                ClassMember::Field(field) => (field.span, &field.annotations, None),
                ClassMember::Constructor(constructor) => {
                    locals.extend(param_names(&constructor.params));
                    (
                        constructor.span,
                        &constructor.annotations,
                        Some(&constructor.params),
                    )
                }
                ClassMember::Method(method) => {
                    locals.extend(type_names(&method.type_params));
                    locals.extend(param_names(&method.params));
                    (method.span, &method.annotations, Some(&method.params))
                }
                ClassMember::Getter(getter) => (getter.span, &getter.annotations, None),
                ClassMember::Setter(setter) => {
                    locals.push(setter.param.name.clone());
                    (setter.span, &setter.annotations, None)
                }
                ClassMember::Operator(operator) => {
                    locals.extend(param_names(&operator.params));
                    (operator.span, &operator.annotations, Some(&operator.params))
                }
                // The header owns the constructor's name and parameters;
                // `this` supplies only its implementation, not another declaration.
                ClassMember::PrimaryConstructorBody(_) | ClassMember::Error(_) => continue,
            };
            if let Some(params) = params {
                self.params(params, &locals, enclosing);
            }
            self.push(span, annotations, locals, enclosing);
        }
    }

    /// Scopes for the doc comments of `params`, which see `locals`.
    fn params(
        &mut self,
        params: &FormalParamList,
        locals: &[String],
        enclosing: &std::rc::Rc<Enclosing>,
    ) {
        for param in &params.params {
            self.push(param.span, &param.annotations, locals.to_vec(), enclosing);
        }
    }

    /// The scope of the declaration `block` documents.
    pub(crate) fn scope_for(&self, block: &DocBlock) -> Option<&DocScope> {
        self.scopes
            .iter()
            .filter(|scope| block.start <= scope.start && scope.start <= block.next_token)
            .max_by_key(|scope| scope.start)
    }
}

/// Resolves parsed references in a file's doc comments.
pub(crate) struct DocResolver<'r, 'a> {
    pub(crate) model: &'r SemanticModel<'a>,
    pub(crate) signatures: &'r SignatureIndex,
    scopes: &'r DocScopes,
    /// SDK sources are loaded, so `dart:core` and `Object` are fully indexed.
    sdk_loaded: bool,
    /// The library sees `dart:core` implicitly.
    implicit_core: bool,
}

impl<'r, 'a> DocResolver<'r, 'a> {
    pub(crate) fn new(
        program: &Program,
        model: &'r SemanticModel<'a>,
        signatures: &'r SignatureIndex,
        scopes: &'r DocScopes,
        sdk_loaded: bool,
    ) -> Self {
        Self {
            model,
            signatures,
            scopes,
            sdk_loaded,
            implicit_core: !program
                .imports
                .iter()
                .any(|import| import.uri.value == "dart:core"),
        }
    }

    /// What each name of `reference` denotes in `scope`. Like the analyzer,
    /// a name after one that is not a prefix, type or extension is absent,
    /// and a three-name reference needs an import prefix first.
    pub(crate) fn resolve<'s>(
        &self,
        reference: &ParsedReference<'s>,
        scope: &DocScope,
    ) -> Vec<DocLookup<'s>> {
        let names = &reference.names;
        let mut lookups = vec![self.simple(names[0].text, scope)];
        // `[new C]` denotes `C`'s unnamed constructor, when it has one.
        if reference.new_keyword
            && names.len() == 1
            && let DocLookup::Found(DocElement::Declaration {
                identity: Some(identity),
                kind: NameIdentity::Type,
            }) = &lookups[0]
            && let ResolvedType::Interface { identity, .. } = self.model.declared_type(identity)
            && let DocLookup::Found(constructor) = self.declared_member(&identity, "new")
        {
            lookups[0] = DocLookup::Found(constructor);
        }
        if let Some(second) = names.get(1) {
            let lookup = if reference.new_keyword {
                DocLookup::Unknown
            } else if names.len() == 3 {
                match &lookups[0] {
                    DocLookup::Found(DocElement::Prefix(_)) => {
                        self.member(&lookups[0], second.text)
                    }
                    DocLookup::Unknown => DocLookup::Unknown,
                    _ => DocLookup::Absent,
                }
            } else {
                self.member(&lookups[0], second.text)
            };
            lookups.push(lookup);
        }
        if let Some(third) = names.get(2) {
            let lookup = match &lookups[1] {
                DocLookup::Found(DocElement::Declaration {
                    identity: Some(identity),
                    kind: NameIdentity::Type,
                }) => match self.model.declared_type(identity) {
                    ResolvedType::Interface { identity, .. } => {
                        self.declared_member(&identity, third.text)
                    }
                    ResolvedType::Unknown => DocLookup::Unknown,
                    _ => DocLookup::Absent,
                },
                DocLookup::Found(DocElement::Extension(name)) => {
                    self.extension_member(name, third.text)
                }
                DocLookup::Found(DocElement::Declaration {
                    kind: NameIdentity::Value,
                    ..
                }) => DocLookup::Absent,
                DocLookup::Found(DocElement::Declaration { .. }) | DocLookup::Unknown => {
                    DocLookup::Unknown
                }
                _ => DocLookup::Absent,
            };
            lookups.push(lookup);
        }
        lookups
    }

    /// A simple name: lexical lookup, then the enclosing type's members.
    fn simple<'s>(&self, name: &'s str, scope: &DocScope) -> DocLookup<'s> {
        if scope.locals.iter().any(|local| local == name) {
            // A top-level function's own doc comment can name its (possibly
            // deprecated) parameters; type parameters stay plain locals.
            if let Some((function, parameters)) = &scope.function
                && parameters.iter().any(|parameter| parameter == name)
            {
                return DocLookup::Found(DocElement::Member {
                    owner: function.clone(),
                    selector: SourceSelector {
                        member: MemberSelector::DECLARATION,
                        parameter: Some(name),
                    },
                });
            }
            return DocLookup::Found(DocElement::Local);
        }
        match scope.enclosing.as_ref() {
            Enclosing::Type { identity, members } => {
                if let Some(&is_static) = members.get(name) {
                    return DocLookup::Found(match identity {
                        Some(owner) if is_static => self.static_element(owner, name),
                        Some(owner) => match self.interface_member(owner, name) {
                            DocLookup::Found(element) => element,
                            DocLookup::Absent | DocLookup::Unknown => DocElement::Opaque,
                        },
                        None => DocElement::Opaque,
                    });
                }
            }
            Enclosing::Extension { members, .. } => {
                if members.contains_key(name) {
                    return DocLookup::Found(DocElement::Opaque);
                }
            }
            Enclosing::None => {}
        }
        match self.model.resolve_doc_name(name) {
            DocName::Declaration { identity, kind } => {
                return DocLookup::Found(DocElement::Declaration { identity, kind });
            }
            DocName::Extension => return DocLookup::Found(DocElement::Extension(name)),
            DocName::Prefix => return DocLookup::Found(DocElement::Prefix(name)),
            DocName::Unknown => return DocLookup::Unknown,
            DocName::Absent => {}
        }
        if !self.sdk_loaded && self.implicit_core && DART_CORE.contains(&name) {
            return DocLookup::Found(DocElement::Declaration {
                identity: None,
                kind: NameIdentity::Unknown,
            });
        }
        match scope.enclosing.as_ref() {
            Enclosing::Type {
                identity: Some(owner),
                ..
            } => self.this_member(owner, name),
            Enclosing::Type { identity: None, .. } => DocLookup::Unknown,
            Enclosing::Extension { on_type, .. } => match on_type {
                ResolvedType::Interface { identity, .. } => self.this_member(identity, name),
                ResolvedType::TypeParameter { bound, .. }
                | ResolvedType::NonNullTypeParameter { bound, .. } => match bound.as_ref() {
                    ResolvedType::Interface { identity, .. } => self.this_member(identity, name),
                    ResolvedType::Unknown => DocLookup::Unknown,
                    _ => DocLookup::Absent,
                },
                ResolvedType::Function { .. } => {
                    self.this_member(&DeclarationIdentity::sdk("dart:core", "Function"), name)
                }
                ResolvedType::Unknown => DocLookup::Unknown,
                _ => DocLookup::Absent,
            },
            Enclosing::None => DocLookup::Absent,
        }
    }

    /// `name` read or written on `this` of type `owner`: an interface member,
    /// a static member of the class chain, or an applicable extension member.
    fn this_member<'s>(&self, owner: &DeclarationIdentity, name: &'s str) -> DocLookup<'s> {
        self.interface_member(owner, name)
            .absent_or(|| self.static_member(owner, name))
            .absent_or(|| {
                let receiver = ResolvedType::interface_identity(owner.clone());
                let applicable = |write| {
                    self.signatures
                        .applicable_extension_access_on_types(&receiver, name, write, self.model)
                        .map(|on_types| !on_types.is_empty())
                };
                match (applicable(false), applicable(true)) {
                    (Some(true), _) | (_, Some(true)) => DocLookup::Found(DocElement::Opaque),
                    (Some(false), Some(false)) => DocLookup::Absent,
                    _ => DocLookup::Unknown,
                }
            })
    }

    /// The instance member `name` of the interface `owner`, inherited ones
    /// (and `Object`'s) included.
    fn interface_member<'s>(&self, owner: &DeclarationIdentity, name: &'s str) -> DocLookup<'s> {
        if self.signatures.type_kind(owner).is_none() && *owner != DeclarationIdentity::OBJECT {
            return DocLookup::Unknown;
        }
        if let Some(fact) = self
            .signatures
            .declared_member_facts(owner, name)
            .iter()
            .find(|fact| !fact.is_static)
        {
            return DocLookup::Found(member_element(owner.clone(), name, fact));
        }
        let Some((facts, complete)) = self.signatures.known_inherited_member_facts(owner, name)
        else {
            return DocLookup::Unknown;
        };
        if let Some(fact) = facts.first() {
            let receiver = ResolvedType::interface_identity(owner.clone());
            let declaring = self
                .signatures
                .resolved_readable_member_owner(&receiver, name, self.model)
                .or_else(|| {
                    self.signatures
                        .resolved_writable_member_owner(&receiver, name, self.model)
                })
                .unwrap_or_else(|| fact.declaring_type.clone());
            return DocLookup::Found(member_element(declaring, name, fact));
        }
        let enum_member = matches!(name, "index" | "name")
            && self.signatures.type_kind(owner) == Some(TypeKind::Enum);
        if !self.sdk_loaded && (OBJECT_MEMBERS.contains(&name) || enum_member) {
            DocLookup::Found(DocElement::Opaque)
        } else if complete {
            DocLookup::Absent
        } else {
            DocLookup::Unknown
        }
    }

    /// The static member `name` of `owner` or a class it extends.
    fn static_member<'s>(&self, owner: &DeclarationIdentity, name: &'s str) -> DocLookup<'s> {
        let mut current = owner.clone();
        let mut visited = HashSet::new();
        while visited.insert(current.clone()) {
            if self.signatures.type_kind(&current).is_none() {
                return DocLookup::Unknown;
            }
            if self.declares_static(&current, name) {
                return DocLookup::Found(self.static_element(&current, name));
            }
            match self.signatures.superclass(&current) {
                Some(ResolvedType::Interface { identity, .. }) => current = identity.clone(),
                Some(_) => return DocLookup::Unknown,
                None => break,
            }
        }
        DocLookup::Absent
    }

    fn declares_static(&self, owner: &DeclarationIdentity, name: &str) -> bool {
        self.signatures
            .declared_member_facts(owner, name)
            .iter()
            .any(|fact| fact.is_static)
            || (self.signatures.type_kind(owner) == Some(TypeKind::Enum)
                && (name == "values" || self.signatures.is_enum_value(owner, name)))
    }

    fn static_element<'s>(&self, owner: &DeclarationIdentity, name: &'s str) -> DocElement<'s> {
        match self
            .signatures
            .declared_member_facts(owner, name)
            .iter()
            .find(|fact| fact.is_static)
        {
            Some(fact) => member_element(owner.clone(), name, fact),
            None => DocElement::Member {
                owner: owner.clone(),
                selector: MemberSelector::new(MemberSelectorKind::Read, true, name).into(),
            },
        }
    }

    /// `name` after the first name of a two-name reference.
    fn member<'s>(&self, first: &DocLookup<'s>, name: &'s str) -> DocLookup<'s> {
        match first {
            DocLookup::Unknown => DocLookup::Unknown,
            DocLookup::Absent => DocLookup::Absent,
            DocLookup::Found(DocElement::Prefix(prefix)) => {
                match self.model.resolve_doc_prefixed(prefix, name) {
                    DocName::Declaration { identity, kind } => {
                        DocLookup::Found(DocElement::Declaration { identity, kind })
                    }
                    DocName::Extension => DocLookup::Found(DocElement::Extension(name)),
                    DocName::Absent => DocLookup::Absent,
                    DocName::Prefix | DocName::Unknown => DocLookup::Unknown,
                }
            }
            DocLookup::Found(DocElement::Declaration { identity, kind }) => {
                match (identity, kind) {
                    (Some(identity), NameIdentity::Type) => {
                        match self.model.declared_type(identity) {
                            ResolvedType::Interface { identity, .. } => self
                                .interface_member(&identity, name)
                                .absent_or(|| self.declared_member(&identity, name)),
                            ResolvedType::Unknown => DocLookup::Unknown,
                            _ => DocLookup::Absent,
                        }
                    }
                    (_, NameIdentity::Value) => DocLookup::Absent,
                    _ => DocLookup::Unknown,
                }
            }
            DocLookup::Found(DocElement::Extension(extension)) => {
                self.extension_member(extension, name)
            }
            DocLookup::Found(
                DocElement::Local | DocElement::Member { .. } | DocElement::Opaque,
            ) => DocLookup::Absent,
        }
    }

    /// A member `owner` itself declares: a method, getter, setter (static or
    /// not) or constructor.
    fn declared_member<'s>(&self, owner: &DeclarationIdentity, name: &'s str) -> DocLookup<'s> {
        let Some(kind) = self.signatures.type_kind(owner) else {
            return DocLookup::Unknown;
        };
        if let Some(fact) = self.signatures.declared_member_facts(owner, name).first() {
            return DocLookup::Found(member_element(owner.clone(), name, fact));
        }
        if self.declares_static(owner, name) {
            return DocLookup::Found(self.static_element(owner, name));
        }
        if self.signatures.has_constructor(owner, name) {
            return DocLookup::Found(DocElement::Member {
                owner: owner.clone(),
                selector: MemberSelector::new(MemberSelectorKind::Constructor, false, name).into(),
            });
        }
        // The representation constructor of an extension type is not indexed.
        if kind == TypeKind::ExtensionType {
            DocLookup::Unknown
        } else {
            DocLookup::Absent
        }
    }

    /// A member a named extension declares; only this file's are known.
    fn extension_member<'s>(&self, extension: &str, name: &'s str) -> DocLookup<'s> {
        match self.scopes.extensions.get(extension) {
            Some(members) if members.contains_key(name) => DocLookup::Found(DocElement::Opaque),
            Some(_) => DocLookup::Absent,
            None => DocLookup::Unknown,
        }
    }
}

fn member_element<'s>(
    owner: DeclarationIdentity,
    name: &'s str,
    fact: &MemberFacts,
) -> DocElement<'s> {
    let kind = match fact.kind {
        MemberKind::Method | MemberKind::Getter => MemberSelectorKind::Read,
        MemberKind::Setter => MemberSelectorKind::Write,
        MemberKind::Field { .. } => MemberSelectorKind::Field,
        MemberKind::Operator => MemberSelectorKind::Operator(fact.positional_parameter_names.len()),
    };
    DocElement::Member {
        owner,
        selector: MemberSelector::new(kind, fact.is_static, name).into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use falcon_analyze::{IdentityIndex, IdentitySource};
    use falcon_dart_parser::parse;
    use std::path::Path;

    fn with_resolver(source: &str, check: impl FnOnce(&DocResolver<'_, '_>, &DocScopes)) {
        let path = Path::new("/project/lib/test.dart");
        let (program, errors) = parse(source);
        assert!(errors.is_empty(), "{errors:?}");
        let identities = IdentityIndex::from_project_files(
            &[IdentitySource {
                path,
                program: &program,
                has_parse_errors: false,
            }],
            &[],
        );
        let model = SemanticModel::new(path, &identities, None);
        let signatures = SignatureIndex::from_program(&program, &model);
        let scopes = DocScopes::new(&program, &model, source);
        let resolver = DocResolver::new(&program, &model, &signatures, &scopes, false);
        check(&resolver, &scopes);
    }

    fn lookup<'s>(
        resolver: &DocResolver<'_, '_>,
        scope: &DocScope,
        text: &'s str,
    ) -> DocLookup<'s> {
        let reference = Reference { text, offset: 0 }.parse().unwrap();
        resolver.resolve(&reference, scope).pop().unwrap()
    }

    #[test]
    fn member_selectors_preserve_kind_dispatch_and_borrow_reference_names() {
        with_resolver(
            "import 'test.dart' as p;\n\
             class C {\n\
               C(); C.named();\n\
               int field = 0;\n\
               int get getter => 0;\n\
               set setter(int value) {}\n\
               static int staticField = 0;\n\
               static void method() {}\n\
               static set staticSetter(int value) {}\n\
               C operator ~() => this;\n\
               C operator +(C other) => this;\n\
               void operator []=(int index, int value) {}\n\
             }\n\
             void probe() {}",
            |resolver, scopes| {
                let scope = scopes.scopes.last().unwrap();
                for (text, kind, is_static, name) in [
                    ("C.field", MemberSelectorKind::Field, false, "field"),
                    ("C.getter", MemberSelectorKind::Read, false, "getter"),
                    ("C.setter", MemberSelectorKind::Write, false, "setter"),
                    (
                        "C.staticField",
                        MemberSelectorKind::Field,
                        true,
                        "staticField",
                    ),
                    ("C.method", MemberSelectorKind::Read, true, "method"),
                    (
                        "C.staticSetter",
                        MemberSelectorKind::Write,
                        true,
                        "staticSetter",
                    ),
                    ("C.new", MemberSelectorKind::Constructor, false, "new"),
                    ("p.C.named", MemberSelectorKind::Constructor, false, "named"),
                    ("C.~", MemberSelectorKind::Operator(0), false, "~"),
                    ("C.+", MemberSelectorKind::Operator(1), false, "+"),
                    ("C.[]=", MemberSelectorKind::Operator(2), false, "[]="),
                ] {
                    let DocLookup::Found(DocElement::Member { owner, selector }) =
                        lookup(resolver, scope, text)
                    else {
                        panic!("did not resolve {text}");
                    };
                    assert_eq!(owner, resolver.model.resolve_name("C").unwrap());
                    assert_eq!(selector, MemberSelector::new(kind, is_static, name).into());
                    assert_eq!(
                        selector.member.name.as_ptr(),
                        text[text.len() - name.len()..].as_ptr(),
                        "{text}",
                    );
                }
                let DocLookup::Found(DocElement::Member { selector, .. }) =
                    lookup(resolver, scope, "new C")
                else {
                    panic!("did not resolve unnamed constructor");
                };
                assert_eq!(
                    selector,
                    MemberSelector::new(MemberSelectorKind::Constructor, false, "new").into(),
                );
            },
        );
    }

    #[test]
    fn function_parameter_selectors_borrow_named_and_positional_names() {
        with_resolver(
            "void function<T>(int positional, {int named = 0}) {}",
            |resolver, scopes| {
                let scope = scopes.scopes.last().unwrap();
                for name in ["positional", "named"] {
                    let DocLookup::Found(DocElement::Member { owner, selector }) =
                        lookup(resolver, scope, name)
                    else {
                        panic!("did not resolve {name}");
                    };
                    assert_eq!(owner, resolver.model.resolve_value("function").unwrap());
                    assert_eq!(
                        selector,
                        SourceSelector {
                            member: MemberSelector::DECLARATION,
                            parameter: Some(name),
                        },
                    );
                    assert_eq!(selector.parameter.unwrap().as_ptr(), name.as_ptr());
                }
                assert!(matches!(
                    lookup(resolver, scope, "T"),
                    DocLookup::Found(DocElement::Local)
                ));
                assert!(matches!(
                    lookup(resolver, scope, "function"),
                    DocLookup::Found(DocElement::Declaration {
                        kind: NameIdentity::Value,
                        ..
                    }),
                ));
            },
        );
    }

    #[test]
    fn extension_setter_and_inherited_members_keep_resolution_behavior() {
        with_resolver(
            "class Base { int field = 0; static void method() {} }\n\
             extension Values on Base { set extensionValue(int value) {} }\n\
             class Child extends Base { void probe() {} }",
            |resolver, scopes| {
                let scope = scopes
                    .scopes
                    .iter()
                    .find(|scope| {
                        matches!(scope.enclosing.as_ref(), Enclosing::Type {
                        identity: Some(identity), ..
                    } if identity.name() == "Child")
                    })
                    .unwrap();
                for (name, kind, is_static) in [
                    ("field", MemberSelectorKind::Field, false),
                    ("method", MemberSelectorKind::Read, true),
                ] {
                    let DocLookup::Found(DocElement::Member { owner, selector }) =
                        lookup(resolver, scope, name)
                    else {
                        panic!("did not resolve {name}");
                    };
                    assert_eq!(owner, resolver.model.resolve_name("Base").unwrap());
                    assert_eq!(selector, MemberSelector::new(kind, is_static, name).into());
                }
                for text in ["extensionValue", "Values.extensionValue"] {
                    assert!(
                        matches!(
                            lookup(resolver, scope, text),
                            DocLookup::Found(DocElement::Opaque),
                        ),
                        "{text}"
                    );
                }
            },
        );
    }
}
