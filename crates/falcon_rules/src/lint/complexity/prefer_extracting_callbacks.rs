//! Flags inline block-body callbacks passed to Widget constructors.
//!
//! The rule uses resolved Flutter identities for both the containing Widget class
//! and each constructed Widget. Calls that cannot be resolved uniquely are left
//! alone; spelling, capitalization, and import-prefix shape are not evidence.

use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, ResolvedType, Rule, SemanticModel, SignatureIndex, Truth,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr, walk_stmt};

#[derive(Default)]
pub struct PreferExtractingCallbacks;

struct Cfg {
    allowed_line_count: Option<usize>,
    ignored: Vec<String>,
}

fn cfg(ctx: &AnalyzeContext) -> Cfg {
    let opts = crate::lint::options::for_rule(ctx, "prefer-extracting-callbacks");
    Cfg {
        allowed_line_count: crate::lint::options::usize(opts, "allowed_line_count"),
        ignored: crate::lint::options::strings(opts, "ignored_named_arguments")
            .into_iter()
            .flatten()
            .map(str::to_string)
            .collect(),
    }
}

impl Rule for PreferExtractingCallbacks {
    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let (Some(identities), Some(signatures)) = (ctx.identities, ctx.signatures) else {
            return Vec::new();
        };
        let model = SemanticModel::new(ctx.file_path, identities, ctx.types);
        let widget = DeclarationIdentity::flutter("Widget");
        let state = DeclarationIdentity::flutter("State");
        let cfg = cfg(ctx);
        let mut diags = Vec::new();
        for declaration in &program.declarations {
            let TopLevelDecl::Class(class) = declaration else {
                continue;
            };
            let Some(identity) = model.resolve_name(&class.name.name) else {
                continue;
            };
            let class_type = ResolvedType::interface_identity(identity);
            if signatures.is_subtype_of(&class_type, &widget, &model) != Truth::Yes
                && signatures.is_subtype_of(&class_type, &state, &model) != Truth::Yes
            {
                continue;
            }
            let mut collector = CallbackCollector {
                model: &model,
                signatures,
                widget: &widget,
                cfg: &cfg,
                source: ctx.source,
                ctx,
                diags: &mut diags,
            };
            for member in &class.members {
                collector.visit_class_member(member);
            }
        }
        diags
    }
}

struct CallbackCollector<'a, 'b> {
    model: &'a SemanticModel<'a>,
    signatures: &'a SignatureIndex,
    widget: &'a DeclarationIdentity,
    cfg: &'a Cfg,
    source: &'a str,
    ctx: &'a AnalyzeContext<'a>,
    diags: &'b mut Vec<Diagnostic>,
}

impl Visitor for CallbackCollector<'_, '_> {
    fn visit_expr(&mut self, node: &Expr) {
        if let Some((constructed, args)) = crate::lint::flutter::construction(node, self.model)
            && self
                .signatures
                .is_subtype_of(&constructed, self.widget, self.model)
                == Truth::Yes
        {
            for argument in &args.positional {
                self.check_callback(argument);
            }
            for argument in &args.named {
                if !self
                    .cfg
                    .ignored
                    .iter()
                    .any(|name| name == &argument.name.name)
                {
                    self.check_callback(&argument.value);
                }
            }
        }
        walk_expr(self, node);
    }
}

impl CallbackCollector<'_, '_> {
    fn check_callback(&mut self, expression: &Expr) {
        let Expr::FuncExpr {
            params, body, span, ..
        } = expression.unparenthesized()
        else {
            return;
        };
        if !matches!(body.as_ref(), FunctionBody::Block(block) if !block.stmts.is_empty())
            || self.is_flutter_builder(params, body)
            || !is_long_enough(span, self.source, self.cfg)
        {
            return;
        }
        self.diags.push(self.ctx.diagnostic(
            "prefer-extracting-callbacks",
            "Prefer extracting the callback to a separate widget method.",
            *span,
        ));
    }

    fn is_flutter_builder(&self, params: &FormalParamList, body: &FunctionBody) -> bool {
        if !body_returns_widget(body, self.model, self.signatures, self.widget) {
            return false;
        }
        let first = params.params.first();
        let Some(first) = first else {
            return true;
        };
        // An untyped `(context, index) {}` is still a builder; only a resolved non-context type rules it out.
        first.param_type.as_ref().is_none_or(|ty| {
            self.model.resolve_type(ty)
                == ResolvedType::interface_identity(DeclarationIdentity::flutter("BuildContext"))
        })
    }
}

fn body_returns_widget(
    body: &FunctionBody,
    model: &SemanticModel<'_>,
    signatures: &SignatureIndex,
    widget: &DeclarationIdentity,
) -> bool {
    match body {
        FunctionBody::Arrow(expression, _) => {
            expression_is_widget(expression, model, signatures, widget)
        }
        FunctionBody::Block(block) => {
            let mut finder = ReturnsWidget {
                model,
                signatures,
                widget,
                found: false,
            };
            for statement in &block.stmts {
                finder.visit_stmt(statement);
            }
            finder.found
        }
        FunctionBody::Native(..) => false,
    }
}

struct ReturnsWidget<'a> {
    model: &'a SemanticModel<'a>,
    signatures: &'a SignatureIndex,
    widget: &'a DeclarationIdentity,
    found: bool,
}

impl Visitor for ReturnsWidget<'_> {
    fn visit_stmt(&mut self, node: &Stmt) {
        if let Stmt::Return(ReturnStmt {
            value: Some(value), ..
        }) = node
            && expression_is_widget(value, self.model, self.signatures, self.widget)
        {
            self.found = true;
        }
        walk_stmt(self, node);
    }

    // Statements only reach expressions through nested closures, whose returns aren't this body's.
    fn visit_expr(&mut self, _node: &Expr) {}
}

fn expression_is_widget(
    expression: &Expr,
    model: &SemanticModel<'_>,
    signatures: &SignatureIndex,
    widget: &DeclarationIdentity,
) -> bool {
    crate::lint::flutter::construction(expression, model)
        .is_some_and(|(ty, _)| signatures.is_subtype_of(&ty, widget, model) == Truth::Yes)
}

fn is_long_enough(span: &Span, source: &str, cfg: &Cfg) -> bool {
    match cfg.allowed_line_count {
        None => true,
        Some(limit) => {
            let end = span.end.min(source.len());
            let Some(text) = source.get(span.start..end) else {
                return false;
            };
            text.bytes().filter(|byte| *byte == b'\n').count() + 1 > limit
        }
    }
}
