use crate::lint::riverpod::is_type;
use falcon_analyze::{AnalyzeContext, RulePreparation};
use falcon_analyze::{SemanticRuleVisitor, SemanticState};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

#[derive(Clone, Copy)]
pub(crate) enum Naming {
    NotifierSuffix,
    ProviderSuffix,
    NotifierFile,
    ProviderFile,
}

pub(crate) fn prepare<'a>(
    program: Program<'a>,
    ctx: &'a AnalyzeContext<'a>,
    naming: Naming,
) -> RulePreparation<'a> {
    if falcon_analyze::is_generated_dart_file(ctx.file_path()) {
        return RulePreparation::Diagnostics(Vec::new());
    }
    RulePreparation::Semantic(Box::new(Collector {
        program,
        ctx,
        naming,
        file_reported: false,
        diagnostics: Vec::new(),
    }))
}

struct Collector<'a> {
    program: Program<'a>,
    ctx: &'a AnalyzeContext<'a>,
    naming: Naming,
    file_reported: bool,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> Collector<'a> {
    fn check(&mut self, name: Identifier<'a>) {
        let (suffix, file_suffix) = match self.naming {
            Naming::NotifierSuffix | Naming::NotifierFile => ("Notifier", "_notifier"),
            Naming::ProviderSuffix | Naming::ProviderFile => ("Provider", "_provider"),
        };
        let message = if matches!(self.naming, Naming::NotifierFile | Naming::ProviderFile) {
            if self.file_reported {
                return;
            }
            let Some(stem) = self
                .ctx
                .file_path()
                .file_stem()
                .and_then(|stem| stem.to_str())
            else {
                return;
            };
            // Generated parts do not own the declarations' source file names.
            if stem.contains('.') || valid_file_stem(stem, file_suffix) {
                return;
            }
            self.file_reported = true;
            format!(
                "Name files containing Riverpod {}s using `snake_case` ending in `{file_suffix}.dart`.",
                suffix.to_ascii_lowercase()
            )
        } else {
            if name.name().ends_with(suffix) {
                return;
            }
            format!(
                "Name Riverpod {}s with a `{suffix}` suffix.",
                suffix.to_ascii_lowercase()
            )
        };
        self.diagnostics
            .push(self.ctx.diagnostic(message, name.span()));
    }

    fn providers(
        &mut self,
        declared_type: Option<DartType<'a>>,
        variables: AstChildren<'a, VarDeclarator<'a>>,
        state: &SemanticState<'a>,
    ) {
        for variable in variables {
            let ty = declared_type.map_or_else(
                || {
                    variable
                        .initializer()
                        .map_or(falcon_analyze::ResolvedType::UNKNOWN, |value| {
                            state.infer(value)
                        })
                },
                |ty| state.model.resolve_type(ty),
            );
            if is_type(self.ctx, state, &ty, PROVIDERS) {
                self.check(variable.name());
            }
        }
    }

    fn generated_provider(&mut self, function: FunctionDecl<'a>, state: &SemanticState<'a>) {
        if let Some(identities) = self.ctx.identities()
            && function.annotations().any(|annotation| {
                falcon_analyze::QualifiedName::from_identifiers(annotation.name())
                    .and_then(|name| state.model.resolve_annotation_before_generation(name))
                    .is_some_and(|identity| {
                        ["riverpod", "Riverpod"].iter().any(|name| {
                            identities.is_package_declaration(
                                &identity,
                                "riverpod_annotation",
                                name,
                            )
                        })
                    })
            })
        {
            self.check(function.name());
        }
    }
}

fn valid_file_stem(stem: &str, suffix: &str) -> bool {
    stem.strip_suffix(suffix).is_some_and(|prefix| {
        !prefix.trim_start_matches('_').is_empty()
            && stem.bytes().enumerate().all(|(index, byte)| {
                byte.is_ascii_lowercase() || byte == b'_' || index > 0 && byte.is_ascii_digit()
            })
    })
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn visit_program_end(&mut self, state: &SemanticState<'a>) {
        for declaration in self.program.declarations() {
            if matches!(self.naming, Naming::NotifierSuffix | Naming::NotifierFile) {
                let (name, superclass) = match declaration {
                    TopLevelDecl::Class(class) => (class.name(), class.extends()),
                    TopLevelDecl::MixinClass(class) => (class.name(), class.extends()),
                    TopLevelDecl::ClassTypeAlias(class) => (class.name(), Some(class.superclass())),
                    _ => continue,
                };
                if superclass.is_some_and(|ty| {
                    is_type(self.ctx, state, &state.model.resolve_type(ty), NOTIFIERS)
                }) {
                    self.check(name);
                }
            } else {
                match declaration {
                    TopLevelDecl::Variable(variable) => {
                        self.providers(variable.var_type(), variable.declarators(), state);
                    }
                    TopLevelDecl::Function(function)
                        if matches!(self.naming, Naming::ProviderFile) =>
                    {
                        self.generated_provider(function, state);
                    }
                    TopLevelDecl::Class(class) => {
                        let Some(resolved) = state.model.resolve_type_name(&[class.name().name()])
                        else {
                            continue;
                        };
                        let falcon_analyze::ResolvedTypeView::Interface { identity, .. } =
                            resolved.view()
                        else {
                            continue;
                        };
                        for member in class.members() {
                            if let ClassMember::Field(field) = member
                                && field.is_static()
                            {
                                for variable in field.declarators() {
                                    if let Some((_, ty)) =
                                        state.signatures.field(identity, variable.name().name())
                                        && is_type(self.ctx, state, ty, PROVIDERS)
                                    {
                                        self.check(variable.name());
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diagnostics
    }
}
const NOTIFIERS: &[&str] = &[
    "Notifier",
    "NotifierBase",
    "BuildlessNotifier",
    "AutoDisposeNotifier",
    "AsyncNotifier",
    "AsyncNotifierBase",
    "BuildlessAsyncNotifier",
    "AutoDisposeAsyncNotifier",
    "StreamNotifier",
    "StreamNotifierBase",
    "AutoDisposeStreamNotifier",
    "AnyNotifier",
    "$Notifier",
    "$AsyncNotifier",
    "$StreamNotifier",
    "StateNotifier",
];

const PROVIDERS: &[&str] = &[
    "ProviderBase",
    "Provider",
    "FutureProvider",
    "StreamProvider",
    "StateProvider",
    "StateNotifierProvider",
    "NotifierProvider",
    "AsyncNotifierProvider",
    "StreamNotifierProvider",
    "AutoDisposeProvider",
    "AutoDisposeFutureProvider",
    "AutoDisposeStreamProvider",
    "AutoDisposeStateProvider",
    "AutoDisposeStateNotifierProvider",
    "AutoDisposeNotifierProvider",
    "AutoDisposeAsyncNotifierProvider",
    "AutoDisposeStreamNotifierProvider",
    "Family",
    "ProviderFamily",
    "FutureProviderFamily",
    "StreamProviderFamily",
    "StateProviderFamily",
    "StateNotifierProviderFamily",
    "NotifierProviderFamily",
    "AsyncNotifierProviderFamily",
    "StreamNotifierProviderFamily",
    "AutoDisposeProviderFamily",
    "AutoDisposeFutureProviderFamily",
    "AutoDisposeStreamProviderFamily",
    "AutoDisposeStateProviderFamily",
    "AutoDisposeStateNotifierProviderFamily",
    "AutoDisposeNotifierProviderFamily",
    "AutoDisposeAsyncNotifierProviderFamily",
    "AutoDisposeStreamNotifierProviderFamily",
];
