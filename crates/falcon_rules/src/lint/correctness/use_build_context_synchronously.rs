//! Do not use Flutter `BuildContext` values across asynchronous gaps.

use falcon_analyze::{
    AnalyzeContext, BuildContextFlowAnalyzer, DeclarationIdentity, ResolvedType, Rule,
    SemanticModel, SignatureIndex, TypeTruth,
};
use falcon_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use falcon_syntax::Program;
use falcon_syntax::ast::*;
use falcon_syntax::visitor::{Visitor, walk_class_decl, walk_program};

pub struct UseBuildContextSynchronously;

impl Rule for UseBuildContextSynchronously {
    fn name(&self) -> &'static str {
        "use-build-context-synchronously"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        if ctx.file_path.components().any(|component| {
            component
                .as_os_str()
                .to_str()
                .is_some_and(|name| name == "test")
        }) {
            return Vec::new();
        }
        let (Some(identities), Some(signatures)) = (ctx.identities, ctx.signatures) else {
            return Vec::new();
        };
        let model = SemanticModel::new(ctx.file_path, identities, ctx.types);
        let mut collector = Collector {
            diagnostics: Vec::new(),
            file: ctx.file_path.to_string_lossy().into_owned(),
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
    file: String,
    model: &'a SemanticModel<'a>,
    signatures: &'a SignatureIndex,
    state_class: bool,
}

impl Collector<'_> {
    fn function(
        &mut self,
        params: &FormalParamList,
        type_params: &[TypeParam],
        body: Option<&FunctionBody>,
    ) {
        let Some(body) = body else {
            return;
        };
        let spans = BuildContextFlowAnalyzer::new(self.model, self.signatures, self.state_class)
            .analyze(params, type_params, body);
        for span in spans {
            self.diagnostics.push(Diagnostic::new(
                "use-build-context-synchronously",
                Severity::Warning,
                "Do not use a BuildContext across an async gap without a matching mounted check.",
                self.file.clone(),
                DiagSpan {
                    start: span.start,
                    end: span.end,
                },
            ));
        }
    }

    fn is_state_class(&self, class: &ClassDecl) -> bool {
        let Some(identity) = self
            .model
            .resolve_name(std::slice::from_ref(&class.name.name))
        else {
            return false;
        };
        let ty = ResolvedType::Interface {
            identity,
            arguments: Vec::new(),
            nullable: false,
            extension_type: false,
        };
        let state = DeclarationIdentity::Package {
            package: "flutter".to_string(),
            name: "State".to_string(),
        };
        self.signatures.is_subtype_of(&ty, &state, self.model) == TypeTruth::Yes
    }
}

impl Visitor for Collector<'_> {
    fn visit_program(&mut self, node: &Program) {
        walk_program(self, node);
    }

    fn visit_class_decl(&mut self, node: &ClassDecl) {
        let saved = self.state_class;
        self.state_class = self.is_state_class(node);
        walk_class_decl(self, node);
        self.state_class = saved;
    }

    fn visit_function_decl(&mut self, node: &FunctionDecl) {
        self.function(&node.params, &node.type_params, node.body.as_ref());
    }

    fn visit_method_decl(&mut self, node: &MethodDecl) {
        self.function(&node.params, &node.type_params, node.body.as_ref());
    }

    fn visit_constructor_decl(&mut self, node: &ConstructorDecl) {
        self.function(&node.params, &[], node.body.as_ref());
    }

    fn visit_getter_decl(&mut self, node: &GetterDecl) {
        self.function(&empty_params(), &[], node.body.as_ref());
    }

    fn visit_setter_decl(&mut self, node: &SetterDecl) {
        let params = FormalParamList {
            positional: vec![FormalParam {
                annotations: node.annotations.clone(),
                is_required: true,
                is_covariant: false,
                is_final: false,
                is_field: false,
                is_super: false,
                param_type: node.param_type.clone(),
                name: node.param.clone(),
                default_value: None,
                type_params: Vec::new(),
                function_params: None,
                span: node.param.span.clone(),
            }],
            optional_positional: Vec::new(),
            named: Vec::new(),
            span: node.span.clone(),
        };
        self.function(&params, &[], node.body.as_ref());
    }
}

fn empty_params() -> FormalParamList {
    FormalParamList {
        positional: Vec::new(),
        optional_positional: Vec::new(),
        named: Vec::new(),
        span: Span::default(),
    }
}
