//! Shared resolved Equatable checks and conservative props-list inspection.

use falcon_analyze::{
    AnalyzeContext, RulePreparation, SemanticModel, SignatureIndex, StructuralRuleVisitor,
    StructuralTraversal, Truth,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::visit;
use falcon_syntax::cst::*;

#[derive(Clone, Copy)]
pub(crate) enum Check {
    Fields,
    Props,
    Mutable,
}

pub(crate) fn prepare<'a>(ctx: &'a AnalyzeContext<'a>, check: Check) -> RulePreparation<'a> {
    let (Some(identities), Some(signatures)) = (ctx.identities(), ctx.signatures()) else {
        return RulePreparation::Diagnostics(Vec::new());
    };
    RulePreparation::Structural(Box::new(Collector {
        ctx,
        model: SemanticModel::new(ctx.file_path(), ctx.parsed(), identities, ctx.types()),
        signatures,
        check,
        listed: Vec::new(),
        diags: Vec::new(),
    }))
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    model: SemanticModel<'a>,
    signatures: &'a SignatureIndex,
    check: Check,
    listed: Vec<&'a str>,
    diags: Vec<Diagnostic>,
}

falcon_analyze::structural_visitor! {
impl<'a> StructuralRuleVisitor<'a> for Collector<'a> {
    fn visit_top_level_decl(&mut self, declaration: TopLevelDecl<'a>) -> StructuralTraversal {
        let (name, members, primary, is_abstract) = match declaration {
            TopLevelDecl::Class(class) => (
                class.name(),
                class.members(),
                class.primary_constructor(),
                class.modifiers().is_abstract
                    || class.modifiers().restriction == Some(ClassRestriction::Sealed),
            ),
            TopLevelDecl::MixinClass(class) => (
                class.name(),
                class.members(),
                class.primary_constructor(),
                class.is_abstract(),
            ),
            _ => return StructuralTraversal::SkipChildren,
        };
        let Some(owner) = self.model.resolve_name(name.name()) else {
            return StructuralTraversal::SkipChildren;
        };
        let equatable = |identity: &falcon_analyze::DeclarationIdentity| {
            identity.is_package("equatable", "Equatable")
                || identity.is_package("equatable", "EquatableMixin")
        };
        if self.signatures.supertype_matches(&owner, &equatable) != Truth::Yes
            || !self.signatures.inherited_concrete_member(&owner, "==")
                .flatten().is_some_and(|member| equatable(&member.declaring_type))
            || members.clone().any(|member| matches!(member, ClassMember::Operator(operator) if operator.op() == "=="))
        {
            return StructuralTraversal::SkipChildren;
        }
        let props = members.clone().find_map(|member| match member {
            ClassMember::Getter(getter) if !getter.is_static() && getter.name().name() == "props" => Some(getter),
            _ => None,
        });
        let has_props_field = matches!(self.check, Check::Props) && (
            members.clone().any(|member| {
                matches!(member, ClassMember::Field(field)
                    if !field.is_static()
                        && field.declarators().any(|field| field.name().name() == "props"))
            }) || (EffectiveMembers {
                declared: members.clone(),
                primary,
                representation: false,
            }.declaring_params()).any(|parameter| parameter.name().name() == "props")
        );
        match self.check {
            Check::Props if !is_abstract && props.is_none() && !has_props_field => {
                self.diags.push(self.ctx.diagnostic(
                    "This Equatable subclass is missing a `props` getter. Override it to define equality.",
                    name.span(),
                ));
            }
            Check::Fields => {
                let Some(getter) = props else { return StructuralTraversal::SkipChildren; };
                let Some(expression) = returned_expression(getter.body()) else {
                    return StructuralTraversal::SkipChildren;
                };
                self.listed.clear();
                if !listed_fields(expression, &mut self.listed) {
                    return StructuralTraversal::SkipChildren;
                }
                let mut missing = false;
                for parameter in (EffectiveMembers { declared: members.clone(), primary, representation: false }.declaring_params()) {
                    missing |= !self.listed.contains(&parameter.name().name());
                }
                for member in members {
                    if let ClassMember::Field(field) = member
                        && !field.is_static() && !field.is_abstract() && !field.is_external() {
                        missing |= field.declarators().any(|field| !self.listed.contains(&field.name().name()));
                    }
                }
                if missing {
                    self.diags.push(self.ctx.diagnostic(
                        "The `props` list omits declared instance fields. Add every field to the list.",
                        getter.name().span(),
                    ));
                }
            }
            Check::Mutable => {
                for parameter in (EffectiveMembers { declared: members.clone(), primary, representation: false }.declaring_params()) {
                    if !parameter.is_final() {
                        self.mutable(parameter.name());
                    }
                }
                for member in members {
                    if let ClassMember::Field(field) = member
                        && !field.is_static() && !field.is_abstract() && !field.is_external()
                        && field.keyword() == VarKeyword::Var {
                        for field in field.declarators() {
                            self.mutable(field.name());
                        }
                    }
                }
            }
            _ => {}
        }
        StructuralTraversal::SkipChildren
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }
}
}

impl Collector<'_> {
    fn mutable(&mut self, name: Identifier<'_>) {
        self.diags.push(self.ctx.diagnostic("Mutable fields can invalidate Equatable equality and hash codes. Make this field final.", name.span()));
    }
}

fn returned_expression(body: Option<FunctionBody<'_>>) -> Option<Expr<'_>> {
    match body? {
        FunctionBody::Arrow(body) => Some(body.expr()),
        FunctionBody::Block(body) => {
            let mut statements = body.stmts();
            match (statements.next(), statements.next()) {
                (Some(Stmt::Return(statement)), None) => statement.value(),
                _ => None,
            }
        }
        _ => None,
    }
}

fn listed_fields<'a>(expression: Expr<'a>, fields: &mut Vec<&'a str>) -> bool {
    let mut collector = PropsFields {
        fields,
        known: true,
    };
    visit::Visitor::visit_expr(&mut collector, expression);
    collector.known
}

struct PropsFields<'f, 'a> {
    fields: &'f mut Vec<&'a str>,
    known: bool,
}

impl<'a> visit::Visitor<'a> for PropsFields<'_, 'a> {
    fn visit_expr(&mut self, expression: Expr<'a>) {
        if !self.known {
            return;
        }
        let expression = expression.unparenthesized();
        match expression {
            Expr::List(_) => visit::walk_expr(self, expression),
            Expr::Field(field)
                if matches!(field.object().unparenthesized(), Expr::Super(_))
                    && field.field().name() == "props" => {}
            Expr::Cascade(cascade) if !cascade.is_null_aware() => {
                for section in cascade.sections() {
                    let mut operations = section.ops();
                    let (Some(CascadeOp::Call(call)), None) =
                        (operations.next(), operations.next())
                    else {
                        self.known = false;
                        return;
                    };
                    let mut arguments = call.args().positional();
                    if call.name().name() != "addAll"
                        || !matches!((arguments.next(), arguments.next()), (Some(_), None))
                    {
                        self.known = false;
                        return;
                    }
                }
                visit::walk_expr(self, expression);
            }
            _ => self.known = false,
        }
    }

    fn visit_collection_element(&mut self, element: CollectionElement<'a>) {
        if !self.known {
            return;
        }
        match element {
            CollectionElement::Expr(expression) => match expression.unparenthesized() {
                Expr::Ident(name) => self.fields.push(name.name()),
                Expr::Field(field) if matches!(field.object().unparenthesized(), Expr::This(_)) => {
                    self.fields.push(field.field().name());
                }
                Expr::IntLit(_) | Expr::StringLit(_) | Expr::BoolLit(_) | Expr::NullLit(_) => {}
                _ => self.known = false,
            },
            CollectionElement::Spread(_) => visit::walk_collection_element(self, element),
            _ => self.known = false,
        }
    }
}
