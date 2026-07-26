//! Flags inline block-body callbacks passed to Widget constructors.
//!
//! The rule uses resolved Flutter identities for both the containing Widget class
//! and each constructed Widget. Calls that cannot be resolved uniquely are left
//! alone; spelling, capitalization, and import-prefix shape are not evidence.

use falcon_analyze::{
    AnalyzeContext, DeclarationIdentity, ResolvedType, Rule, SemanticModel, SignatureIndex,
    TypeTruth,
};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_expr};

pub struct PreferExtractingCallbacks;

struct Cfg {
    allowed_line_count: Option<usize>,
    ignored: Vec<String>,
}

fn cfg(ctx: &AnalyzeContext) -> Cfg {
    let opts = crate::meta::meta_for("prefer-extracting-callbacks")
        .and_then(|m| ctx.rule_options(m.group, "prefer-extracting-callbacks"));
    Cfg {
        allowed_line_count: opts
            .and_then(|o| o.get("allowed_line_count"))
            .and_then(|v| v.as_u64())
            .map(|v| v as usize),
        ignored: opts
            .and_then(|o| o.get("ignored_named_arguments"))
            .and_then(|v| v.as_array())
            .map(|values| {
                values
                    .iter()
                    .filter_map(|value| value.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default(),
    }
}

impl Rule for PreferExtractingCallbacks {
    fn name(&self) -> &'static str {
        "prefer-extracting-callbacks"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let (Some(identities), Some(signatures)) = (ctx.identities, ctx.signatures) else {
            return Vec::new();
        };
        let model = SemanticModel::new(ctx.file_path, identities, ctx.types);
        let widget = flutter_identity("Widget");
        let state = flutter_identity("State");
        let cfg = cfg(ctx);
        let mut diags = Vec::new();
        for declaration in &program.declarations {
            let TopLevelDecl::Class(class) = declaration else {
                continue;
            };
            let Some(identity) = model.resolve_name(std::slice::from_ref(&class.name.name)) else {
                continue;
            };
            let class_type = interface(identity);
            if signatures.is_subtype_of(&class_type, &widget, &model) != TypeTruth::Yes
                && signatures.is_subtype_of(&class_type, &state, &model) != TypeTruth::Yes
            {
                continue;
            }
            let mut collector = CallbackCollector {
                model: &model,
                signatures,
                widget: &widget,
                cfg: &cfg,
                source: ctx.source,
                file: ctx.file_path.to_string_lossy().into_owned(),
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
    file: String,
    diags: &'b mut Vec<Diagnostic>,
}

impl Visitor for CallbackCollector<'_, '_> {
    fn visit_expr(&mut self, node: &Expr) {
        if let Some((constructed, _, args)) = construction(node, self.model)
            && self
                .signatures
                .is_subtype_of(&constructed, self.widget, self.model)
                == TypeTruth::Yes
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
        } = expression
        else {
            return;
        };
        if !matches!(body.as_ref(), FunctionBody::Block(block) if !block.stmts.is_empty())
            || self.is_flutter_builder(params, body)
            || !is_long_enough(span, self.source, self.cfg)
        {
            return;
        }
        self.diags.push(Diagnostic::new(
            "prefer-extracting-callbacks",
            Severity::Warning,
            "Prefer extracting the callback to a separate widget method.",
            self.file.clone(),
            DiagSpan {
                start: span.start,
                end: span.end,
            },
        ));
    }

    fn is_flutter_builder(&self, params: &FormalParamList, body: &FunctionBody) -> bool {
        if !body_returns_widget(body, self.model, self.signatures, self.widget) {
            return false;
        }
        let first = params
            .positional
            .first()
            .or_else(|| params.optional_positional.first())
            .or_else(|| params.named.first());
        let Some(first) = first else {
            return true;
        };
        first.param_type.as_ref().is_some_and(|ty| {
            self.model.resolve_type(ty) == interface(flutter_identity("BuildContext"))
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
        FunctionBody::Block(block) => block.stmts.iter().any(|statement| match statement {
            Stmt::Return(ReturnStmt {
                value: Some(value), ..
            }) => expression_is_widget(value, model, signatures, widget),
            _ => false,
        }),
        FunctionBody::Native(..) => false,
    }
}

fn expression_is_widget(
    expression: &Expr,
    model: &SemanticModel<'_>,
    signatures: &SignatureIndex,
    widget: &DeclarationIdentity,
) -> bool {
    construction(expression, model)
        .is_some_and(|(ty, _, _)| signatures.is_subtype_of(&ty, widget, model) == TypeTruth::Yes)
}

fn construction<'a>(
    expression: &'a Expr,
    model: &SemanticModel<'_>,
) -> Option<(ResolvedType, String, &'a ArgList)> {
    match expression {
        Expr::New {
            dart_type, args, ..
        } => Some((
            model.resolve_type(dart_type).with_nullable(false),
            "new".to_string(),
            args,
        )),
        Expr::Call { callee, args, .. } => {
            let mut segments = expression_segments(callee)?;
            if let Some(identity) = model.resolve_name(&segments) {
                return Some((interface(identity), "new".to_string(), args));
            }
            let constructor = segments.pop()?;
            let identity = model.resolve_name(&segments)?;
            Some((interface(identity), constructor, args))
        }
        _ => None,
    }
}

fn expression_segments(expression: &Expr) -> Option<Vec<String>> {
    let mut current = expression;
    let mut segments = Vec::new();
    loop {
        match current {
            Expr::Ident(identifier) => {
                segments.push(identifier.name.clone());
                segments.reverse();
                return Some(segments);
            }
            Expr::Field { object, field, .. } => {
                segments.push(field.name.clone());
                current = object;
            }
            _ => return None,
        }
    }
}

fn interface(identity: DeclarationIdentity) -> ResolvedType {
    ResolvedType::Interface {
        identity,
        arguments: Vec::new(),
        nullable: false,
        extension_type: false,
    }
}

fn flutter_identity(name: &str) -> DeclarationIdentity {
    DeclarationIdentity::Package {
        package: "flutter".to_string(),
        name: name.to_string(),
    }
}

fn is_long_enough(span: &Span, source: &str, cfg: &Cfg) -> bool {
    match cfg.allowed_line_count {
        None => true,
        Some(limit) => {
            let end = span.end.min(source.len());
            source[span.start..end]
                .bytes()
                .filter(|byte| *byte == b'\n')
                .count()
                + 1
                > limit
        }
    }
}
