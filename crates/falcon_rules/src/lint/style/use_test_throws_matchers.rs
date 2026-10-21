//! Prefers `throwsA` matchers over `try { ...; fail(...); } catch (...) {}`.
//!
//! A `try` statement with a single `catch` clause, no `finally`, and a body
//! that ends in a call to the test framework's `fail` hand-rolls what
//! `expect(() => ..., throwsA(...))` states directly, and with worse failure
//! messages. Like the official rule, only the top-level `fail` declared in
//! `package:test_api/src/frontend/expect.dart` counts. Current `test_api`
//! releases no longer declare it there (`package:test`, `test_api`, and
//! `flutter_test` re-export `package:matcher`'s `fail`), so only code using
//! such an old `test_api` is reported; a local or member function named
//! `fail` never is. The declaring library is identified through the package
//! configuration, including when `test_api` is a dependency.
//!
//! ```dart
//! try {
//!   parse('');
//!   fail('Expected a FormatException');  // BAD
//! } on FormatException {}
//!
//! expect(() => parse(''), throwsA(isA<FormatException>())); // GOOD
//! ```
//!
//! Adopted from the official `use_test_throws_matchers` lint.

use falcon_analyze::{
    AnalyzeContext, IdentityIndex, LibraryId, QualifiedName, Rule, RulePreparation,
};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::*;

use falcon_analyze::semantic_scope::{SemanticRuleVisitor, SemanticState};

#[derive(Default)]
pub struct UseTestThrowsMatchers;

impl Rule for UseTestThrowsMatchers {
    fn prepare<'a>(
        &self,
        _program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        RulePreparation::Semantic(Box::new(Collector {
            ctx,
            expect_library: expect_library(ctx),
            diags: Vec::new(),
        }))
    }
}

struct Collector<'a> {
    ctx: &'a AnalyzeContext<'a>,
    /// `package:test_api/src/frontend/expect.dart`, when it is indexed.
    expect_library: Option<LibraryId>,
    diags: Vec<Diagnostic>,
}

impl<'a> SemanticRuleVisitor<'a> for Collector<'a> {
    fn finish(self: Box<Self>) -> Vec<Diagnostic> {
        self.diags
    }

    fn visit_stmt(&mut self, node: Stmt<'a>, state: &SemanticState<'a>) {
        let Stmt::TryCatch(statement) = node else {
            return;
        };
        if statement.catches().len() != 1 || statement.finally().is_some() {
            return;
        }
        if let Some(Stmt::Expr(last)) = statement.body().stmts().last()
            && let Some(library) = self.expect_library
            && let Some(identities) = self.ctx.identities()
            && is_test_fail(last.expr(), library, identities, state)
        {
            self.diags.push(self.ctx.diagnostic("Use the 'throwsA' matcher instead of using 'fail' when there is no exception thrown.", last.span()));
        }
    }
}

/// The indexed library of `package:test_api/src/frontend/expect.dart`.
fn expect_library(ctx: &AnalyzeContext<'_>) -> Option<LibraryId> {
    let configured = ctx
        .environment()
        .and_then(|environment| environment.package_config(ctx.file_path()))
        .and_then(|config| config.resolve_package_uri("package:test_api/src/frontend/expect.dart"));
    let path = configured.or_else(|| {
        let package = crate::cross_file::enclosing_package(ctx.file_path())?;
        (package.name == "test_api").then(|| package.lib_root.join("src/frontend/expect.dart"))
    })?;
    let identities = ctx.identities()?;
    let file = identities.file(&path)?;
    Some(identities.library(file))
}

/// Whether `expression` invokes the top-level `fail` declared in `library`.
fn is_test_fail<'a>(
    expression: Expr<'a>,
    library: LibraryId,
    identities: &IdentityIndex,
    state: &SemanticState<'a>,
) -> bool {
    let Expr::Call(call) = expression.unparenthesized() else {
        return false;
    };
    let name = match call.callee().unparenthesized() {
        Expr::Ident(name) => {
            let shadowed = state.environment.is_bound(name.name())
                || state.enclosing_type.as_ref().is_some_and(|receiver| {
                    state
                        .signatures
                        .resolved_member(receiver, name.name(), &state.model)
                        .is_some()
                });
            if shadowed {
                return false;
            }
            QualifiedName::from(name.name())
        }
        Expr::Field(field) if !field.is_null_safe() => match field.object().unparenthesized() {
            Expr::Ident(prefix) if !state.environment.is_bound(prefix.name()) => {
                QualifiedName::prefixed(prefix.name(), field.field().name())
            }
            _ => return false,
        },
        _ => return false,
    };
    state.model.resolve_value(name).is_some_and(|identity| {
        identity.name() == "fail" && identities.declaration_library(&identity) == Some(library)
    })
}
