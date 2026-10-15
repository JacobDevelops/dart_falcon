//! Flags `dart:html` APIs that can inject untrusted HTML or URLs.
//!
//! Assigning `href` on an `AnchorElement`, `src` on an `EmbedElement`,
//! `IFrameElement`, or `ScriptElement`, or `srcdoc` on an `IFrameElement`;
//! calling `createFragment` or `setInnerHtml` on an `Element`, or `open` on a
//! `Window`; and invoking the `Element.html` or `DocumentFragment.html`
//! constructors all bypass sanitization and are a common XSS vector. A
//! receiver whose static type is `dynamic` is flagged too, since it may be any
//! of these. Receivers use flow-promoted types at the current use site;
//! receivers whose type cannot be established are left alone.
//!
//! ```dart
//! import 'dart:html';
//!
//! void show(AnchorElement link, String url) {
//!   link.href = url;                    // BAD
//!   window.open(url, 'popup');          // BAD
//!   Element.html('<p>$url</p>');        // BAD
//! }
//! ```
//!
//! UPSTREAM-NOOP: `unsafe_html` is absent from current upstream, having been
//! removed from the Dart SDK in 3.7 alongside the deprecation of `dart:html`.
//! Falcon retains its unsafe-HTML implementation.

use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, QualifiedName, ResolvedType, Rule, Truth,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;
use falcon_syntax::ast::*;

use crate::lint::semantic_scope::{SemanticRuleVisitor, SemanticState, visit_program};

const HTML: &str = "dart:html";

/// The `dart:html` classes falcon resolves that are `Element` or extend it.
const ELEMENTS: &[&str] = &[
    "Element",
    "HtmlElement",
    "AnchorElement",
    "EmbedElement",
    "IFrameElement",
    "ScriptElement",
    "DivElement",
    "SpanElement",
    "ParagraphElement",
    "BodyElement",
    "TemplateElement",
];

#[derive(Default)]
pub struct UnsafeHtml;

impl Rule for UnsafeHtml {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let Some(state) = SemanticState::new(program, ctx) else {
            return Vec::new();
        };
        let mut collector = Collector {
            ctx,
            diags: Vec::new(),
        };
        visit_program(&mut collector, program, state);
        collector.diags
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    diags: Vec<Diagnostic>,
}

impl SemanticRuleVisitor for Collector<'_> {
    fn visit_expr(&mut self, node: &Expr, state: &SemanticState<'_>) {
        match node {
            Expr::Assign { target, span, .. } => {
                let property = match target.as_ref() {
                    Expr::Ident(name) if !state.environment.is_bound(&name.name) => state
                        .enclosing_type
                        .clone()
                        .map(|receiver| (receiver, &name.name)),
                    Expr::Field { object, field, .. } => {
                        Some((receiver_type(object, state), &field.name))
                    }
                    _ => None,
                };
                if let Some((receiver, name)) = property {
                    self.attribute(&receiver, name, *span, state);
                }
            }
            Expr::Call { callee, span, .. } => {
                let method = match callee.as_ref() {
                    Expr::Ident(name) if !state.environment.is_bound(&name.name) => state
                        .enclosing_type
                        .clone()
                        .map(|receiver| (receiver, &name.name)),
                    Expr::Field { object, field, .. } => {
                        Some((receiver_type(object, state), &field.name))
                    }
                    _ => None,
                };
                if let Some((receiver, name)) = method {
                    self.method(&receiver, name, *span, state);
                }
                self.constructor(node, state);
            }
            Expr::New { .. } => self.constructor(node, state),
            Expr::Cascade {
                object, sections, ..
            } => {
                let receiver = receiver_type(object, state);
                for section in sections {
                    match section.ops.as_slice() {
                        [CascadeOp::Assign(target, _, _)] => {
                            if let Expr::Ident(name) = target.as_ref() {
                                self.attribute(&receiver, &name.name, section.span, state);
                            }
                        }
                        [CascadeOp::Call(name, _, _), ..] => {
                            self.method(&receiver, &name.name, section.span, state);
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
}

impl Collector<'_> {
    fn attribute(
        &mut self,
        receiver: &ResolvedType,
        name: &str,
        span: Span,
        state: &SemanticState<'_>,
    ) {
        let classes: &[&str] = match name {
            "href" => &["AnchorElement"],
            "src" => &["EmbedElement", "IFrameElement", "ScriptElement"],
            "srcdoc" => &["IFrameElement"],
            _ => return,
        };
        if *receiver == ResolvedType::Dynamic
            || classes
                .iter()
                .any(|class| extends_html(receiver, class, state))
        {
            self.diags.push(self.ctx.diagnostic(
                "unsafe-html",
                format!("Avoid unsafe HTML APIs (assigning \"{name}\" attribute)."),
                span,
            ));
        }
    }

    fn method(
        &mut self,
        receiver: &ResolvedType,
        name: &str,
        span: Span,
        state: &SemanticState<'_>,
    ) {
        let class = match name {
            "createFragment" | "setInnerHtml" => "Element",
            "open" => "Window",
            _ => return,
        };
        if *receiver == ResolvedType::Dynamic || extends_html(receiver, class, state) {
            self.diags.push(self.ctx.diagnostic(
                "unsafe-html",
                format!("Avoid unsafe HTML APIs (calling the '{name}' method of {class})."),
                span,
            ));
        }
    }

    fn constructor(&mut self, node: &Expr, state: &SemanticState<'_>) {
        let Some((identity, "html", _)) =
            crate::lint::flutter::constructor_invocation(node, &state.model)
        else {
            return;
        };
        let class = if identity.is_sdk(HTML, "DocumentFragment") {
            "DocumentFragment"
        } else if identity.is_sdk(HTML, "Element") {
            "Element"
        } else {
            return;
        };
        self.diags.push(self.ctx.diagnostic(
            "unsafe-html",
            format!("Avoid unsafe HTML APIs (calling the 'html' constructor of {class})."),
            *node.span(),
        ));
    }
}

/// The static type of a member receiver, treating the `dart:html` top-level
/// `window` getter as a `Window`.
fn receiver_type(object: &Expr, state: &SemanticState<'_>) -> ResolvedType {
    let ty = state.infer(object);
    if ty != ResolvedType::Unknown {
        return ty;
    }
    let name = match object {
        Expr::Ident(name) if !state.environment.is_bound(&name.name) => {
            QualifiedName::from(&name.name)
        }
        Expr::Field {
            object,
            field,
            is_null_safe: false,
            ..
        } => match object.as_ref() {
            Expr::Ident(prefix) if !state.environment.is_bound(&prefix.name) => {
                QualifiedName::prefixed(&prefix.name, &field.name)
            }
            _ => return ty,
        },
        _ => return ty,
    };
    match state.model.resolve_sdk_member(name) {
        Some(identity) if identity.is_sdk(HTML, "window") => {
            ResolvedType::interface_identity(DeclarationIdentity::sdk(HTML, "Window"))
        }
        _ => ty,
    }
}

/// Whether `ty` is the `dart:html` class `class` or a known subtype of it.
fn extends_html(ty: &ResolvedType, class: &'static str, state: &SemanticState<'_>) -> bool {
    let ResolvedType::Interface { identity, .. } = ty else {
        return false;
    };
    let targets: &[&'static str] = if class == "Element" {
        ELEMENTS
    } else {
        &[class]
    };
    targets.iter().any(|target| {
        identity.is_sdk(HTML, target)
            || state.signatures.is_subtype_of(
                ty,
                &DeclarationIdentity::sdk(HTML, target),
                &state.model,
            ) == Truth::Yes
    })
}
