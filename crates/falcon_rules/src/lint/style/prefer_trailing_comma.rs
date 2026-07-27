//! Flags multi-line argument lists that are missing a trailing comma.
//!
//! A trailing comma is required only when the argument list is already broken
//! across lines — when the last significant token sits on an earlier line than
//! the closing `)`. This mirrors the Dart 3.x tall-style formatter.

use falcon_analyze::{AnalyzeContext, Rule};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{
    Visitor, walk_annotation, walk_constructor_decl, walk_enum_decl, walk_expr,
};

pub struct PreferTrailingComma;

impl Rule for PreferTrailingComma {
    fn name(&self) -> &'static str {
        "prefer-trailing-comma"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut collector = Collector {
            diags: Vec::new(),
            ctx,
        };
        collector.visit_program(program);
        collector.diags
    }
}

fn line_of(source: &str, offset: usize) -> usize {
    let offset = offset.min(source.len());
    source.as_bytes()[..offset]
        .iter()
        .filter(|&&b| b == b'\n')
        .count()
}

/// Returns the real closing `)` when a trailing comma is required.
fn needs_trailing_comma(source: &str, args_span: &Span) -> Option<usize> {
    let bytes = source.as_bytes();
    let end = args_span.end.min(source.len());
    let mut i = args_span.start;
    while i < end && bytes[i] != b'(' {
        i += 1;
    }
    if i >= end {
        return None;
    }

    let mut depth = 0usize;
    let mut last_sig = None;
    let mut close = None;
    while i < end {
        let byte = bytes[i];
        match byte {
            b'\'' | b'"' => {
                last_sig = Some(i);
                let quote = byte;
                i += 1;
                while i < end {
                    match bytes[i] {
                        b'\\' => i += 2,
                        q if q == quote => {
                            last_sig = Some(i);
                            i += 1;
                            break;
                        }
                        _ => i += 1,
                    }
                }
            }
            b'/' if i + 1 < end && bytes[i + 1] == b'/' => {
                while i < end && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if i + 1 < end && bytes[i + 1] == b'*' => {
                i += 2;
                while i + 1 < end && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                    i += 1;
                }
                i += 2;
            }
            b'(' => {
                depth += 1;
                last_sig = Some(i);
                i += 1;
            }
            b')' => {
                depth -= 1;
                if depth == 0 {
                    close = Some(i);
                    break;
                }
                last_sig = Some(i);
                i += 1;
            }
            _ => {
                if !byte.is_ascii_whitespace() {
                    last_sig = Some(i);
                }
                i += 1;
            }
        }
    }

    match (last_sig, close) {
        (Some(last), Some(close))
            if bytes[last] != b',' && line_of(source, last) < line_of(source, close) =>
        {
            Some(close)
        }
        _ => None,
    }
}

struct Collector<'a, 'ctx> {
    diags: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'ctx>,
}

impl Collector<'_, '_> {
    fn check_args(&mut self, args: &ArgList) {
        if args.positional.is_empty() && args.named.is_empty() {
            return;
        }
        let Some(close) = needs_trailing_comma(self.ctx.source, &args.span) else {
            return;
        };
        self.diags.push(Diagnostic::new(
            "prefer-trailing-comma",
            Severity::Warning,
            "Add a trailing comma to the argument list",
            self.ctx.file_path.to_string_lossy().into_owned(),
            DiagSpan {
                start: close,
                end: close + 1,
            },
        ));
    }
}

impl Visitor for Collector<'_, '_> {
    fn visit_annotation(&mut self, node: &Annotation) {
        if let Some(args) = &node.args {
            self.check_args(args);
        }
        walk_annotation(self, node);
    }

    fn visit_constructor_decl(&mut self, node: &ConstructorDecl) {
        for initializer in &node.initializers {
            match initializer {
                ConstructorInitializer::SuperCall { args, .. }
                | ConstructorInitializer::ThisCall { args, .. } => self.check_args(args),
                ConstructorInitializer::FieldInit { .. }
                | ConstructorInitializer::Assert { .. } => {}
            }
        }
        walk_constructor_decl(self, node);
    }

    fn visit_enum_decl(&mut self, node: &EnumDecl) {
        for variant in &node.variants {
            if let Some(args) = &variant.args {
                self.check_args(args);
            }
        }
        walk_enum_decl(self, node);
    }

    fn visit_expr(&mut self, node: &Expr) {
        match node {
            Expr::Call { args, .. } | Expr::New { args, .. } => self.check_args(args),
            Expr::Cascade { sections, .. } => {
                for section in sections {
                    for op in &section.ops {
                        if let CascadeOp::Call(_, _, args) = op {
                            self.check_args(args);
                        }
                    }
                }
            }
            _ => {}
        }
        walk_expr(self, node);
    }
}
