//! Do not use Flutter `BuildContext` values across asynchronous gaps.

use falcon_analyze::{
    AnalyzeContext, BuildContextFlowAnalyzer, DeclarationIdentity, ResolvedType, Rule,
    SemanticModel, SignatureIndex, Truth,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::Program;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_class_decl};

pub struct UseBuildContextSynchronously;

impl Rule for UseBuildContextSynchronously {
    fn name(&self) -> &'static str {
        "use-build-context-synchronously"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        if is_test_file(ctx.file_path) {
            return Vec::new();
        }
        let (Some(identities), Some(signatures)) = (ctx.identities, ctx.signatures) else {
            return Vec::new();
        };
        let model = SemanticModel::new(ctx.file_path, identities, ctx.types);
        let mut collector = Collector {
            diagnostics: Vec::new(),
            ctx,
            model: &model,
            signatures,
            state_class: false,
        };
        collector.visit_program(program);
        collector.diagnostics
    }
}

struct Collector<'a> {
    diagnostics: Vec<Diagnostic>,
    ctx: &'a AnalyzeContext<'a>,
    model: &'a SemanticModel<'a>,
    signatures: &'a SignatureIndex,
    state_class: bool,
}

impl Collector<'_> {
    fn function(
        &mut self,
        params: Option<&FormalParamList>,
        type_params: &[TypeParam],
        body: Option<&FunctionBody>,
    ) {
        let Some(body) = body else {
            return;
        };
        let spans = BuildContextFlowAnalyzer::new(self.model, self.signatures, self.state_class)
            .analyze(params, type_params, body);
        for span in spans {
            self.diagnostics.push(self.ctx.diagnostic(
                "use-build-context-synchronously",
                "Do not use a BuildContext across an async gap without a matching mounted check.",
                span,
            ));
        }
    }

    fn is_state_class(&self, class: &ClassDecl) -> bool {
        let Some(identity) = self.model.resolve_name(&class.name.name) else {
            return false;
        };
        let ty = ResolvedType::Interface {
            identity,
            arguments: Vec::new(),
            nullable: false,
            extension_type: false,
        };
        let state = DeclarationIdentity::Package {
            package: "flutter",
            name: "State",
        };
        self.signatures.is_subtype_of(&ty, &state, self.model) == Truth::Yes
    }
}

impl Visitor for Collector<'_> {
    fn visit_class_decl(&mut self, node: &ClassDecl) {
        let saved = self.state_class;
        self.state_class = self.is_state_class(node);
        walk_class_decl(self, node);
        self.state_class = saved;
    }

    fn visit_function_decl(&mut self, node: &FunctionDecl) {
        self.function(node.params(), &node.type_params, node.body.as_ref());
    }

    fn visit_method_decl(&mut self, node: &MethodDecl) {
        self.function(Some(&node.params), &node.type_params, node.body.as_ref());
    }

    fn visit_constructor_decl(&mut self, node: &ConstructorDecl) {
        self.function(Some(&node.params), &[], node.body.as_ref());
    }

    fn visit_getter_decl(&mut self, node: &GetterDecl) {
        self.function(None, &[], node.body.as_ref());
    }

    fn visit_setter_decl(&mut self, node: &SetterDecl) {
        let params = FormalParamList {
            params: vec![FormalParam {
                annotations: node.annotations.clone(),
                kind: ParamKind::Positional,
                is_covariant: false,
                is_final: false,
                initializing: None,
                param_type: node.param_type.clone(),
                name: node.param.clone(),
                default_value: None,
                type_params: Vec::new(),
                function_params: None,
                span: node.param.span,
            }],
            span: node.span,
        };
        self.function(Some(&params), &[], node.body.as_ref());
    }
}

/// Test code drives `BuildContext` through pumped frames the analyzer can't
/// model, so the rule skips it. Directory matching is relative to the package
/// root — an absolute path component named `test` (a home directory, say) must
/// not disable the rule for the whole project.
fn is_test_file(path: &std::path::Path) -> bool {
    if path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with("_test.dart"))
    {
        return true;
    }
    let Some(pubspec) = crate::cross_file::enclosing_pubspec(path) else {
        return false;
    };
    let Some(root) = pubspec.parent() else {
        return false;
    };
    let canonical = falcon_analyze::canonical_or_lexical(path);
    canonical
        .strip_prefix(root)
        .is_ok_and(|relative| relative.components().any(|c| c.as_os_str() == "test"))
}
