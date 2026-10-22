use super::*;
use crate::resolve::IdentitySource;
use falcon_dart_parser::parse;

fn with_model(source: &str, test: impl FnOnce(Program<'_>, &SemanticModel<'_>)) {
    let path = PathBuf::from("/project/lib/main.dart");
    let falcon_dart_parser::ParseResult { tree, errors } = parse(source);
    assert!(errors.is_empty(), "parse errors: {errors:?}");
    let parsed = Arc::new(crate::ParsedFile {
        source: source.into(),
        tree,
    });
    let sources = [IdentitySource {
        path: &path,
        parsed: &parsed,
        has_parse_errors: false,
    }];
    let identities = IdentityIndex::from_project_files(&sources, &[]);
    let types = TypeIndex::from_program(parsed.program());
    let model = SemanticModel::new(&path, &parsed, &identities, Some(&types));
    test(parsed.program(), &model);
}

#[test]
fn long_binary_chains_in_closures_and_conditions_use_normal_stack() {
    let chain = " + 0".repeat(100_000);
    for source in [
        format!("int Function() f() => () {{ int? x; (x = 1){chain}; return x; }};"),
        format!("int Function() f() => () {{ int? x; if (x!{chain} > 0) {{}} return x; }};"),
    ] {
        with_model(&source, |program, model| {
            let signatures = SignatureIndex::from_program(program, model);
            let TopLevelDecl::Function(function) = program.declarations().next().unwrap() else {
                panic!("expected function");
            };
            let Some(FunctionBody::Arrow(body)) = function.body() else {
                panic!("expected arrow body");
            };
            assert_eq!(
                TypeEnvironment::new().infer_with_signatures(
                    body.expr(),
                    model,
                    &signatures,
                    &TypeParameterScope::default(),
                ),
                model.resolve_type(function.return_type().unwrap()),
            );
        });
    }
}
