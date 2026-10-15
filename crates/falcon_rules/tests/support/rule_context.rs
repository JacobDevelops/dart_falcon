use falcon_analyze::{
    AnalyzeContext, IdentityIndex, IdentitySource, LibrarySource, Rule, SemanticModel, SignatureIndex,
    TypeIndex,
};
use falcon_config::FalconConfig;
use falcon_dart_parser::parse;
use std::path::Path;

pub fn count(rule: &dyn Rule, source: &str, with_types: bool) -> usize {
    let falcon_dart_parser::ParseResult { program, syntax, .. } = parse(source);
    let config = FalconConfig::default();
    let path = Path::new("t.dart");
    let identities = IdentityIndex::from_project_files(
        &[IdentitySource {
            path,
            program: &program,
            has_parse_errors: false,
        }],
        &[],
    );
    let types = with_types.then(|| {
        TypeIndex::from_resolved_files(
            [(
                path,
                LibrarySource {
                    program: &program,
                    has_parse_errors: false,
                    has_unresolved_parts: false,
                },
            )],
            &identities,
        )
    });
    let model = SemanticModel::new(path, &identities, types.as_ref());
    let signatures = SignatureIndex::from_program(&program, &model);
    let mut ctx = AnalyzeContext::new(path, source, &config, &syntax)
        .with_identities(&identities)
        .with_signatures(&signatures);
    if let Some(types) = &types {
        ctx = ctx.with_types(types);
    }
    rule.analyze(&program, &ctx).len()
}
