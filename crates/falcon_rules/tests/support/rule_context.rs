use falcon_analyze::{
    AnalyzeContext, IdentityIndex, IdentitySource, LibrarySource, Rule, RuleAnalysis, SemanticModel,
    SignatureIndex, TypeIndex,
};
use falcon_config::FalconConfig;
use falcon_dart_parser::parse;
use std::path::Path;

pub fn count(name: &'static str, rule: &dyn Rule, source: &str, with_types: bool) -> usize {
    let falcon_dart_parser::ParseResult { tree, .. } = parse(source);
    let parsed = std::sync::Arc::new(falcon_analyze::ParsedFile { source: source.into(), tree });
    let program = parsed.program();
    let config = FalconConfig::default();
    let path = Path::new("t.dart");
    let identities = IdentityIndex::from_project_files(
        &[IdentitySource {
            path,
            parsed: &parsed,
            has_parse_errors: false,
        }],
        &[],
    );
    let types = with_types.then(|| {
        TypeIndex::from_resolved_files(
            [(
                path,
                LibrarySource {
                    program,
                    has_parse_errors: false,
                    has_unresolved_parts: false,
                },
            )],
            &identities,
        )
    });
    let model = SemanticModel::new(path, &parsed, &identities, types.as_ref());
    let signatures = SignatureIndex::from_program(program, &model);
    let mut ctx = AnalyzeContext::new(path, &config, &parsed)
        .with_identities(&identities)
        .with_signatures(&signatures);
    if let Some(types) = &types {
        ctx = ctx.with_types(types);
    }
    rule.analyze(name, program, &ctx).len()
}
