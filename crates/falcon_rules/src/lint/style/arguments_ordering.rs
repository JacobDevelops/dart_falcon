//! Named arguments follow their resolved parameter declaration order by default.
//! Unknown signatures are skipped. `order: ascending` or `order: descending`
//! explicitly selects alphabetical ordering; `alphabetize: true` selects ascending.

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};
use falcon_analyze::{
    AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal,
};
use falcon_analyze::{QualifiedName, ResolvedSignature, ResolvedType, resolve_new_constructor};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::visit::{self, Visitor};
use falcon_syntax::cst::*;

use super::ordering::Order;


#[derive(Default)]
pub struct ArgumentsOrdering;

impl Rule for ArgumentsOrdering {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let order = Order::configured(ctx, Self::NAME).or_else(|| {
            crate::lint::options::for_rule(ctx, Self::NAME)?
                .get("alphabetize")?
                .as_bool()?
                .then_some(Order::Ascending)
        });
        let Some(order) = order else {
            return RulePreparation::Semantic(Box::new(DeclarationCollector {
                ctx,
                program,
                diagnostics: Vec::new(),
            }));
        };
        RulePreparation::Structural(Box::new(Collector {
            ctx,
            order,
            diagnostics: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    order: Order,
    diagnostics: Vec<Diagnostic>,
}

impl Collector<'_> {
    fn check(&mut self, args: ArgList<'_>) {
        self.order.check_names(
            self.ctx,
            args.named().map(|argument| Some(argument.name())),
            &mut self.diagnostics,
        );
    }
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> { self.diagnostics }

    fn visit_annotation(&mut self, node: Annotation<'a>) -> StructuralTraversal {
        if let Some(args) = node.args() {
            self.check(args);
        }
        StructuralTraversal::Continue
    }

    fn visit_constructor_initializer(&mut self, node: ConstructorInitializer<'a>) -> StructuralTraversal {
        match node {
            ConstructorInitializer::SuperCall(call) => self.check(call.args()),
            ConstructorInitializer::ThisCall(call) => self.check(call.args()),
            _ => {}
        }
        StructuralTraversal::Continue
    }

    fn visit_enum_decl(&mut self, node: EnumDecl<'a>) -> StructuralTraversal {
        for variant in node.variants() {
            if let Some(args) = variant.args() {
                self.check(args);
            }
        }
        StructuralTraversal::Continue
    }

    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {
        match node {
            Expr::Call(call) => self.check(call.args()),
            Expr::New(construction) => self.check(construction.args()),
            Expr::Cascade(cascade) => {
                for section in cascade.sections() {
                    for op in section.ops() {
                        match op {
                            CascadeOp::Call(call) => self.check(call.args()),
                            CascadeOp::Invoke(call) => self.check(call.args()),
                            _ => {}
                        }
                    }
                }
            }
            _ => {}
        }
        StructuralTraversal::Continue
    }
}
}

struct DeclarationCollector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    program: Program<'a>,
    diagnostics: Vec<Diagnostic>,
}

impl DeclarationCollector<'_> {
    fn check(&mut self, args: ArgList<'_>, signature: &ResolvedSignature) {
        let names = &signature.named_parameter_names;
        if args.named().len() < 2 || names.len() != signature.named.len() {
            return;
        }
        // An invalid or incompletely resolved invocation has no reliable order.
        if args
            .named()
            .any(|arg| !names.iter().any(|name| name == arg.name().name()))
        {
            return;
        }
        let mut previous = None;
        for argument in args.named() {
            let current = names.iter().position(|name| name == argument.name().name());
            if previous
                .zip(current)
                .is_some_and(|(previous, current)| previous > current)
            {
                self.diagnostics.push(self.ctx.diagnostic("Reorder named arguments to follow the parameter declaration order.", argument.name().span()));
            }
            previous = current;
        }
    }

    fn constructor(
        &mut self,
        receiver: Option<&ResolvedType>,
        name: &str,
        args: ArgList<'_>,
        state: &SemanticState<'_>,
    ) {
        if let Some(ResolvedType::Interface { identity, .. }) = receiver
            && let Some(signature) = state.signatures.constructor(identity, name)
        {
            self.check(args, signature);
        }
    }
}

impl<'a> SemanticRuleVisitor<'a> for DeclarationCollector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }

    fn visit_expr(&mut self, node: Expr<'a>, state: &SemanticState<'a>) {
        match node {
            Expr::Call(call) => {
                if let Some(target) = state.call_target(call.callee(), call.type_args()) {
                    self.check(call.args(), &target.signature);
                }
            }
            Expr::New(node) => {
                if let Some((identity, name)) =
                    resolve_new_constructor(node.dart_type(), node.constructor_name(), &state.model)
                    && let Some(signature) = state.signatures.constructor(&identity, name)
                {
                    self.check(node.args(), signature);
                }
            }
            _ => {}
        }
    }

    fn visit_constructor_ref(&mut self, node: ConstructorRef<'a>, state: &SemanticState<'a>) {
        for initializer in node.initializers {
            let (receiver, name, args) = match initializer {
                ConstructorInitializer::SuperCall(call) => {
                    (state.superclass.as_ref(), call.call_name(), call.args())
                }
                ConstructorInitializer::ThisCall(call) => {
                    (state.enclosing_type.as_ref(), call.call_name(), call.args())
                }
                _ => continue,
            };
            self.constructor(
                receiver,
                name.map_or("new", |name| name.name()),
                args,
                state,
            );
        }
    }

    fn visit_cascade_operation(
        &mut self,
        operation: CascadeOp<'a>,
        receiver: &ResolvedType,
        _section: Span,
        state: &SemanticState<'a>,
    ) {
        if let CascadeOp::Call(call) = operation
            && let Some((signature, _)) =
                state
                    .signatures
                    .resolved_member(receiver, call.name().name(), &state.model)
        {
            self.check(call.args(), signature);
        }
    }

    fn visit_program_end(&mut self, state: &SemanticState<'a>) {
        // Annotation and enum arguments are declaration syntax, outside expression callbacks.
        let program = self.program;
        DeclarationArguments {
            collector: self,
            state,
        }
        .visit_program(program);
    }
}

struct DeclarationArguments<'c, 's, 'a> {
    collector: &'c mut DeclarationCollector<'a>,
    state: &'s SemanticState<'a>,
}

impl<'a> Visitor<'a> for DeclarationArguments<'_, '_, 'a> {
    fn visit_annotation(&mut self, node: Annotation<'a>) {
        let names = node.name();
        let explicit_constructor = node.constructor_name();
        let resolve = |name: Option<QualifiedName<'a>>| {
            name.and_then(|name| self.state.model.resolve_name(name))
        };
        let target = resolve(QualifiedName::from_identifiers(names.clone()))
            .map(|identity| {
                (
                    identity,
                    explicit_constructor.map_or("new", |name| name.name()),
                )
            })
            .or_else(|| {
                if explicit_constructor.is_some() || names.clone().count() < 2 {
                    return None;
                }
                let constructor = names.clone().last()?;
                let identity = resolve(QualifiedName::from_identifiers(
                    names.clone().take(names.count() - 1),
                ))?;
                Some((identity, constructor.name()))
            });
        if let Some(args) = node.args()
            && let Some((identity, constructor)) = target
            && let Some(signature) = self.state.signatures.constructor(&identity, constructor)
        {
            self.collector.check(args, signature);
        }
        visit::walk_annotation(self, node);
    }

    fn visit_enum_decl(&mut self, node: EnumDecl<'a>) {
        if let Some(identity) = self.state.model.resolve_name(node.name().name()) {
            for variant in node.variants() {
                if let Some(args) = variant.args()
                    && let Some(signature) = self.state.signatures.constructor(
                        &identity,
                        variant.constructor_name().map_or("new", |name| name.name()),
                    )
                {
                    self.collector.check(args, signature);
                }
            }
        }
        visit::walk_enum_decl(self, node);
    }
}
