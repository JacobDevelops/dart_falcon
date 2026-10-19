//! Extract inline string literals into named variables or constants.
//! Exempts direct literal initializers, defaults, annotations, constant collections,
//! indices, asserts, and direct string arguments of regex/URI/error constructors,
//! common string operations and `ignored_invocations` (default ["print", "debugPrint"]).
//! `allow_only_once` (false) allows values occurring only once across expression/pattern literals.
//! Literal patterns are exempt from diagnostics but still count; directive strings do not count.

use std::collections::HashMap;

use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use crate::lint::style::conditional_uri_does_not_exist::uri_value;

#[derive(Default)]
pub struct NoMagicString;

impl Rule for NoMagicString {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let options = crate::lint::options::for_rule(ctx, "no-magic-string");
        let allow_only_once = options
            .and_then(|options| options.get("allow_only_once"))
            .and_then(|value| value.as_bool())
            .unwrap_or(false);
        let ignored = options
            .and_then(|options| options.get("ignored_invocations"))
            .and_then(|value| value.as_array())
            .map(|values| values.iter().filter_map(|value| value.as_str()).collect())
            .unwrap_or_else(|| vec!["print", "debugPrint"]);
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            allow_only_once,
            ignored,
            literals: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    allow_only_once: bool,
    ignored: Vec<&'a str>,
    literals: Vec<(String, Span, bool)>,
}

impl Collector<'_> {
    fn exempt(&self, literal: StringLitNode<'_>) -> bool {
        let mut child = literal.syntax();
        while let Some(parent) = child.parent() {
            match parent.kind() {
                NodeKind::Annotation
                | NodeKind::FormalParam
                | NodeKind::AssertStmt
                | NodeKind::AssertInitializer
                | NodeKind::EnumVariant
                | NodeKind::LiteralPattern => return true,
                NodeKind::VarDeclarator
                    if child == literal.syntax()
                        || Expr::cast(child).is_some_and(|expr| {
                            expr.unparenthesized().syntax() == literal.syntax()
                        }) =>
                {
                    return true;
                }
                _ => {}
            }
            if LocalVarDecl::cast(parent).is_some_and(|decl| decl.keyword() == VarKeyword::Const)
                || TopLevelVarDecl::cast(parent)
                    .is_some_and(|decl| decl.keyword() == VarKeyword::Const)
                || FieldDecl::cast(parent).is_some_and(|decl| decl.keyword() == VarKeyword::Const)
            {
                return true;
            }
            if matches!(CascadeOp::cast(parent),
                Some(CascadeOp::Index(index)) if index.index().syntax() == child)
                || matches!(CascadeOp::cast(parent),
                    Some(CascadeOp::IndexAssign(assign)) if assign.target().syntax() == child)
                || MapEntry::cast(parent).is_some_and(|entry| entry.key().syntax() == child)
            {
                return true;
            }
            if let Some(expr) = Expr::cast(parent) {
                match expr {
                    Expr::List(list) if list.is_const() => return true,
                    Expr::Set(set) if set.is_const() => return true,
                    Expr::Map(map) if map.is_const() => return true,
                    Expr::Index(index) if index.index().syntax() == child => return true,
                    Expr::Call(call)
                        if call
                            .args()
                            .positional()
                            .chain(call.args().named().map(|argument| argument.value()))
                            .any(|argument| {
                                argument.unparenthesized().syntax() == literal.syntax()
                            }) =>
                    {
                        let name = match call.callee().unparenthesized() {
                            Expr::Ident(name) => Some(name.name()),
                            Expr::Field(field) => Some(field.field().name()),
                            _ => None,
                        };
                        if name.is_some_and(|name| {
                            self.ignored.contains(&name) || exempt_invocation(name)
                        }) {
                            return true;
                        }
                        if let Expr::Field(field) = call.callee().unparenthesized()
                            && matches!(field.object().unparenthesized(), Expr::Ident(name) if exempt_invocation(name.name()))
                        {
                            return true;
                        }
                    }
                    Expr::New(new) => {
                        if new.is_const()
                            || (new
                                .args()
                                .positional()
                                .chain(new.args().named().map(|argument| argument.value()))
                                .any(|argument| {
                                    argument.unparenthesized().syntax() == literal.syntax()
                                })
                                && matches!(new.dart_type(), DartType::Named(name)
                            if name.segments().last().is_some_and(|name| exempt_invocation(name.name()))))
                        {
                            return true;
                        }
                    }
                    Expr::FuncExpr(_) => break,
                    _ => {}
                }
            }
            child = parent;
        }
        false
    }
}

fn exempt_invocation(name: &str) -> bool {
    matches!(
        name,
        "RegExp"
            | "Uri"
            | "GlobalKey"
            | "Exception"
            | "split"
            | "join"
            | "replaceAll"
            | "replaceFirst"
            | "replaceRange"
            | "startsWith"
            | "endsWith"
            | "contains"
            | "indexOf"
            | "lastIndexOf"
            | "padLeft"
            | "padRight"
    ) || name.ends_with("Error")
        || name.ends_with("Exception")
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        let mut counts = HashMap::new();
        if self.allow_only_once {
            for (value, _, _) in &self.literals {
                *counts.entry(value.as_str()).or_insert(0usize) += 1;
            }
        }
        self.literals.iter()
            .filter(|(value, _, exempt)| !exempt && (!self.allow_only_once || counts.get(value.as_str()).copied().unwrap_or(0) > 1))
            .map(|(_, span, _)| self.ctx.diagnostic("no-magic-string",
                "Extract this magic string into a named variable or constant.", *span))
            .collect()
    }

    fn visit_string_lit(&mut self, literal: StringLitNode<'a>) -> StructuralTraversal {
        if literal.syntax().parent().is_some_and(|parent| matches!(parent.kind(),
            NodeKind::ImportDirective | NodeKind::ExportDirective | NodeKind::PartDirective
                | NodeKind::PartOfDirective | NodeKind::ConfigurableUri))
        {
            return StructuralTraversal::Continue;
        }
        if literal.interpolations().next().is_none() {
            let value = uri_value(literal);
            if !value.is_empty() {
                let exempt = self.exempt(literal);
                if self.allow_only_once || !exempt {
                    self.literals.push((value, literal.span(), exempt));
                }
            }
        }
        StructuralTraversal::Continue
    }
}
}
