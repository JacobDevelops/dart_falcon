//! Flags ordinary comments that contain recognizable Dart statements.
//!
//! Delete obsolete code rather than keeping it in comments. This deliberately
//! conservative heuristic recognizes declarations, calls, assignments, and
//! return/throw statements with code punctuation. Documentation comments,
//! fenced examples, prose, and TODO/FIXME notes are not reported.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::TokenKind;
use falcon_syntax::cst::*;

#[derive(Default)]
pub struct AvoidCommentedOutCode;

impl Rule for AvoidCommentedOutCode {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let mut diagnostics = Vec::<Diagnostic>::new();
        let mut fenced = false;
        for token in ctx.tree().root(ctx.source()).tokens() {
            if !token.kind().is_comment() {
                if !token.is_trivia() {
                    fenced = false;
                }
                continue;
            }
            if token.kind() == TokenKind::DocComment {
                continue;
            }
            let text = token.text();
            let content = text
                .strip_prefix("//")
                .or_else(|| {
                    text.strip_prefix("/*")
                        .and_then(|text| text.strip_suffix("*/"))
                })
                .unwrap_or(text);
            let mut code = false;
            for line in content.lines() {
                let line = line.trim().trim_start_matches('*').trim();
                if line.starts_with("```") || line.starts_with("~~~") {
                    fenced = !fenced;
                    continue;
                }
                if !fenced && is_code(line) {
                    code = true;
                }
            }
            if code {
                diagnostics.push(ctx.diagnostic("Remove commented-out code; version control preserves obsolete implementations.", token.span()));
            }
        }
        RulePreparation::Diagnostics(diagnostics)
    }
}

fn identifier(text: &str) -> bool {
    let mut characters = text.chars();
    characters
        .next()
        .is_some_and(|character| character == '_' || character == '$' || character.is_alphabetic())
        && characters
            .all(|character| character == '_' || character == '$' || character.is_alphanumeric())
}

fn is_code(line: &str) -> bool {
    let line = line.trim();
    if line.starts_with("TODO") || line.starts_with("FIXME") || line.contains('`') {
        return false;
    }
    if let Some(header) = line.strip_suffix('{') {
        let header = header.trim();
        if let Some((signature, parameters)) = header.split_once('(') {
            let mut words = signature.split_whitespace();
            return matches!((words.next(), words.next(), words.next()),
                (Some(ty), Some(name), None) if identifier(ty) && identifier(name))
                && parameters.ends_with(')');
        }
        return header.strip_prefix("class ").is_some_and(identifier);
    }
    let Some(statement) = line.strip_suffix(';') else {
        return false;
    };
    let statement = statement.trim();
    if let Some(value) = statement
        .strip_prefix("return ")
        .or_else(|| statement.strip_prefix("throw "))
    {
        return value.contains('(')
            || value == "null"
            || value == "true"
            || value == "false"
            || value.parse::<i64>().is_ok();
    }
    if let Some((left, right)) = statement.split_once(" = ") {
        let mut words = left.split_whitespace();
        let Some(first) = words.next() else {
            return false;
        };
        let declaration = match (words.next(), words.next(), words.next()) {
            (None, None, None) => identifier(first),
            (Some(name), None, None) => {
                matches!(
                    first,
                    "var"
                        | "final"
                        | "const"
                        | "int"
                        | "double"
                        | "num"
                        | "bool"
                        | "String"
                        | "Object"
                        | "dynamic"
                ) && identifier(name)
            }
            (Some(ty), Some(name), None) => {
                matches!(first, "final" | "const") && identifier(ty) && identifier(name)
            }
            _ => false,
        };
        return declaration
            && !right.is_empty()
            && (right.starts_with(['\'', '"', '[', '{'])
                || right.parse::<i64>().is_ok()
                || matches!(right, "true" | "false" | "null")
                || call(right));
    }
    call(statement)
}

fn call(statement: &str) -> bool {
    let Some((callee, arguments)) = statement.split_once('(') else {
        return false;
    };
    callee.split('.').all(identifier) && arguments.ends_with(')')
}
