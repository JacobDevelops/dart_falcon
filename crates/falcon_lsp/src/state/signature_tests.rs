use super::*;

fn state(root: &Path) -> LspState {
    let config = root.join("falcon.json");
    std::fs::write(
        &config,
        r#"{"linter":{"rules":{"recommended":false,"style":{"prefer-final-fields":"warn","use-late-for-private-fields-and-variables":"warn"},"suspicious":{"avoid-ignoring-return-values":"warn"}}},"cross-file":{"enabled":false}}"#,
    )
    .unwrap();
    LspState::new_from(Some(config), |_| None, root.to_path_buf())
}

fn uri(path: &Path) -> String {
    format!("file://{}", path.display())
}

fn signature(source: &str) -> u64 {
    let parsed = try_parse_for_package(source, None).unwrap();
    assert!(parsed.errors.is_empty(), "{source}: {:?}", parsed.errors);
    ParsedFile {
        source: source.into(),
        tree: parsed.tree,
    }
    .declaration_signature()
}

fn assert_shifted_invocation_types(
    state: &LspState,
    path: &Path,
    parsed: &Arc<ParsedFile>,
    indexes: &SemanticIndexes,
) {
    use falcon_analyze::{
        ResolvedType, SemanticRuleVisitor, SemanticState, TypeParameterScope, visit_program,
    };
    use falcon_syntax::cst::{Expr, TopLevelDecl};

    struct Check<'a> {
        source: &'a str,
        integer: ResolvedType,
        alias: ResolvedType,
        parameter: ResolvedType,
        checked: usize,
    }

    impl<'a> SemanticRuleVisitor<'a> for Check<'_> {
        fn visit_expr(&mut self, expression: Expr<'a>, semantic: &SemanticState<'a>) {
            let span = expression.span();
            match &self.source[span.start..span.end] {
                "Box<int>().identity(1)" => {
                    assert_eq!(semantic.infer(expression), self.integer);
                    self.checked += 1;
                }
                "Screen().callback()" | "callback()" => {
                    let inferred = semantic.infer(expression);
                    assert_eq!(inferred, self.alias);
                    assert_eq!(
                        inferred.function_alias_identity(),
                        self.alias.function_alias_identity()
                    );
                    self.checked += 1;
                }
                "identity(v)" => {
                    // Mix the current tree's header key with the retained member's
                    // return type: stale source offsets cannot substitute this T.
                    assert_eq!(semantic.infer(expression), self.parameter);
                    self.checked += 1;
                }
                _ => {}
            }
        }
    }

    let ctx = AnalyzeContext::new(path, &state.config, parsed)
        .with_identities(&indexes.identities)
        .with_types(&indexes.types)
        .with_signatures(&indexes.signatures);
    let semantic = SemanticState::new(parsed.program(), &ctx).unwrap();
    let class = parsed
        .program()
        .declarations()
        .find_map(|declaration| match declaration {
            TopLevelDecl::Class(class) if class.name().name() == "Box" => Some(class),
            _ => None,
        })
        .unwrap();
    let mut parameters = TypeParameterScope::default();
    parameters.push(class.type_params(), &semantic.model);
    let mut check = Check {
        source: &parsed.source,
        integer: semantic.model.core_type("int", false),
        alias: semantic.model.resolve_type_name(&["Builder"]).unwrap(),
        parameter: parameters.lookup("T").unwrap(),
        checked: 0,
    };
    visit_program(&mut check, parsed.program(), semantic);
    assert_eq!(check.checked, 3);
}

#[test]
fn declaration_signature_skips_bodies_and_shifted_type_offsets() {
    assert_eq!(signature("int f() => 1;"), signature("int f() => 2222;"));
    // A body's own named types do not affect the retained declaration index.
    assert_eq!(
        signature("void f() { final x = <int>[]; }"),
        signature("void f() { final x = <String>[]; }")
    );
    for declaration in [
        "Alias make() => 1;",
        "class Box<T> { T value(T input) => input; }",
        "T identity<T>(T value) => value;",
    ] {
        let before = format!("typedef Alias = int; void f() {{}} {declaration}");
        let after = format!("typedef Alias = int; void f() {{ /* shift */ }} {declaration}");
        assert_eq!(signature(&before), signature(&after), "{declaration}");
    }
}

#[test]
fn body_only_part_edits_refresh_open_library_warnings_without_importers_or_rebuilds() {
    for (rule, owner, before, after) in [
        (
            "prefer-final-fields",
            "part 'part.dart'; class Counter { int _count = 0; int get count => _count; }",
            "part of 'owner.dart'; void reset(Counter c) {}",
            "part of 'owner.dart'; void reset(Counter c) { c._count = 1; }",
        ),
        (
            "use-late-for-private-fields-and-variables",
            "part 'part.dart'; int? _count; int read() => _count!;",
            "part of 'owner.dart'; void reset() { _count = 1; }",
            "part of 'owner.dart'; void reset() { _count = null; }",
        ),
    ] {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        let owner_path = root.join("owner.dart");
        let part_path = root.join("part.dart");
        let importer_path = root.join("main.dart");
        let importer = "import 'owner.dart'; void main() {}";
        std::fs::write(&owner_path, owner).unwrap();
        std::fs::write(&part_path, before).unwrap();
        std::fs::write(&importer_path, importer).unwrap();
        let owner_uri = uri(&owner_path);
        let part_uri = uri(&part_path);
        let importer_uri = uri(&importer_path);
        let mut state = state(root);
        state.open(&owner_uri, owner.into(), Some(1));
        state.open(&part_uri, before.into(), Some(1));
        state.open(&importer_uri, importer.into(), Some(1));
        assert!(
            state
                .analyze(&owner_uri)
                .iter()
                .any(|diagnostic| diagnostic.rule == rule)
        );
        let builds = state.semantic_snapshot_build_count;
        let topologies = state.semantic_topology_build_count;
        let importer_analyses = state.documents[&importer_uri].analyze_count;
        for (version, source, warning) in [(2, after, false), (3, before, true)] {
            assert!(state.change(&part_uri, source.into(), Some(version)));
            let published = state.analyze_affected_many(std::slice::from_ref(&part_uri));
            assert_eq!(
                published.iter().map(|(uri, _)| uri).collect::<HashSet<_>>(),
                HashSet::from([&owner_uri, &part_uri])
            );
            let diagnostics = &published
                .iter()
                .find(|(uri, _)| uri == &owner_uri)
                .unwrap()
                .1;
            assert_eq!(
                diagnostics.iter().any(|diagnostic| diagnostic.rule == rule),
                warning,
                "{rule}: {diagnostics:?}"
            );
            assert_eq!(state.semantic_snapshot_build_count, builds);
            assert_eq!(state.semantic_topology_build_count, topologies);
            assert_eq!(
                state.documents[&importer_uri].analyze_count,
                importer_analyses
            );
            assert_eq!(
                state.documents[&part_uri].declaration_signature,
                signature(source)
            );
        }
        // Closing an unsaved body restores sibling diagnostics, not importer work.
        state.change(&part_uri, after.into(), Some(4));
        state.analyze_affected_many(std::slice::from_ref(&part_uri));
        state.close(&part_uri);
        assert_eq!(state.affected_open_uris(&part_uri), vec![owner_uri.clone()]);
        assert!(
            state
                .analyze(&owner_uri)
                .iter()
                .any(|diagnostic| diagnostic.rule == rule)
        );
        assert_eq!(state.semantic_snapshot_build_count, builds);
        state.close(&owner_uri);
        assert!(state.affected_open_uris(&owner_uri).is_empty());
    }
}

#[test]
fn body_shift_before_typed_build_and_generic_class_reuses_indexes_and_positions() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("main.dart");
    let source = |body: &str| {
        format!(
            "typedef Builder = int Function();\n\
             class Screen {{\n\
               void initState() {{ {body} }}\n\
               int build() => 1;\n\
               Builder callback() => () => 1;\n\
             }}\n\
             class Box<T> {{ T identity(T value) => value; T again(T v) {{ return identity(v); }} }}\n\
             void main() {{\n\
               Screen().build();\n\
               Box<int>().identity(1);\n\
               Screen().callback();\n\
             }}\n"
        )
    };
    let before = source("");
    std::fs::write(&path, &before).unwrap();
    let uri = uri(&path);
    let mut state = state(directory.path());
    state.resolve_semantics = true;
    state.open(&uri, before.clone(), Some(1));
    let builds = state.semantic_snapshot_build_count;
    let topologies = state.semantic_topology_build_count;
    let external = state.external_index_build_count;
    for (version, source) in [
        (
            2,
            source("final values = <String>[];\n// shifted declarations\n"),
        ),
        (3, source("final values = <int>[1, 2, 3];")),
        (4, before),
    ] {
        assert!(state.change(&uri, source.clone(), Some(version)));
        assert!(state.semantic_snapshot.is_some());
        let published = state.analyze_affected_many(std::slice::from_ref(&uri));
        assert_eq!(published.len(), 1, "{published:?}");
        let diagnostics: Vec<_> = published[0]
            .1
            .iter()
            .filter(|diagnostic| diagnostic.rule == "avoid-ignoring-return-values")
            .collect();
        assert_eq!(diagnostics.len(), 3, "{diagnostics:?}");
        for call in [
            "Screen().build()",
            "Box<int>().identity(1)",
            "Screen().callback()",
        ] {
            let start = source.find(call).unwrap();
            let diagnostic = diagnostics
                .iter()
                .find(|diagnostic| diagnostic.span.start == start)
                .unwrap_or_else(|| panic!("missing current diagnostic at {call}: {diagnostics:?}"));
            assert!(diagnostic.span.end <= start + call.len());
            assert_eq!(
                diagnostic.line,
                u32::try_from(
                    source[..start]
                        .bytes()
                        .filter(|&byte| byte == b'\n')
                        .count()
                        + 1
                )
                .unwrap()
            );
        }
        let snapshot = state.semantic_snapshot.as_ref().unwrap();
        assert_shifted_invocation_types(
            &state,
            &path,
            &state.documents[&uri].parsed,
            &snapshot.indexes,
        );
        assert_eq!(state.semantic_snapshot_build_count, builds);
        assert_eq!(state.semantic_topology_build_count, topologies);
        assert_eq!(state.external_index_build_count, external);
        assert_eq!(
            state.documents[&uri].declaration_signature,
            signature(&source)
        );
    }
}

#[test]
fn cross_file_only_body_reuse_preserves_types_after_save_close_and_same_text_reread() {
    let directory = tempfile::tempdir().unwrap();
    let config = directory.path().join("falcon.json");
    std::fs::write(&config, "{}").unwrap();
    let path = directory.path().join("main.dart");
    let source = |body| {
        format!(
            "typedef Builder = int Function(); void initState() {{ {body} }} \
             Builder callback() => () => 1; \
             class Box<T> {{ T identity(T value) => value; T again(T v) {{ return identity(v); }} }} \
             void main() {{ Box<int>().identity(1); callback(); }}"
        )
    };
    let before = source("");
    std::fs::write(&path, &before).unwrap();
    let uri = uri(&path);
    let mut state = LspState::new_from(Some(config), |_| None, directory.path().to_path_buf());
    state.resolve_semantics = false;
    state.cross_file_resolve_semantics = true;
    state.open(&uri, before, Some(1));
    let (files, complete) = state.collect_project_files(ProjectView::CrossFile);
    assert!(complete);
    state.ensure_cross_file_indexes(&files);
    let builds = state.cross_file_index_build_count;
    assert_shifted_invocation_types(
        &state,
        &path,
        &files[0].parsed,
        &state.cross_file_index_cache.as_ref().unwrap().indexes,
    );

    let after = source("final values = <String>[]; // shift later declarations\n");
    state.change(&uri, after.clone(), Some(2));
    let edited = Arc::clone(&state.documents[&uri].parsed);
    let (files, complete) = state.collect_project_files(ProjectView::CrossFile);
    assert!(complete);
    state.ensure_cross_file_indexes(&files);
    assert_shifted_invocation_types(
        &state,
        &path,
        &edited,
        &state.cross_file_index_cache.as_ref().unwrap().indexes,
    );
    assert_eq!(state.cross_file_index_build_count, builds);

    std::fs::write(&path, &after).unwrap();
    state.save(&uri, None);
    state.close(&uri);
    let (files, complete) = state.collect_project_files(ProjectView::CrossFile);
    assert!(complete);
    let reread = &files.iter().find(|file| file.path == path).unwrap().parsed;
    assert_eq!(reread.source, edited.source);
    assert!(!Arc::ptr_eq(reread, &edited));
    state.ensure_cross_file_indexes(&files);
    assert_shifted_invocation_types(
        &state,
        &path,
        reread,
        &state.cross_file_index_cache.as_ref().unwrap().indexes,
    );
    assert_eq!(state.cross_file_index_build_count, builds);
    assert_eq!(state.semantic_snapshot_build_count, 0);
    assert_eq!(state.semantic_topology_build_count, 0);
    assert!(state.semantic_snapshot.is_none());
}

#[cfg(unix)]
#[test]
fn saving_retargeted_body_only_alias_keeps_old_dependents() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let old = root.join("old.dart");
    let new = root.join("new.dart");
    let alias = root.join("alias.dart");
    let caller = root.join("main.dart");
    std::fs::write(&old, "int api() => 1;").unwrap();
    std::fs::write(&new, "void api() {}").unwrap();
    std::fs::write(&caller, "import 'old.dart'; void main() { api(); }").unwrap();
    std::os::unix::fs::symlink(&old, &alias).unwrap();
    let alias_uri = uri(&alias);
    let caller_uri = uri(&caller);
    let mut state = state(root);
    state.open(
        &caller_uri,
        std::fs::read_to_string(&caller).unwrap(),
        Some(1),
    );
    state.open(&alias_uri, "int api() => 1;".into(), Some(1));
    state.affected_open_uris(&alias_uri);
    state.change(&alias_uri, "int api() => 2222;".into(), Some(2));
    assert!(state.body_only_changes.contains_key(&alias_uri));
    std::fs::remove_file(&alias).unwrap();
    std::os::unix::fs::symlink(&new, &alias).unwrap();
    state.save(&alias_uri, None);
    assert!(state.affected_open_uris(&alias_uri).contains(&caller_uri));
    assert_eq!(state.documents[&alias_uri].path, canonical_or_lexical(&new));
}

#[cfg(unix)]
#[test]
fn closing_retargeted_body_only_alias_keeps_its_original_library_siblings() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let owner = root.join("owner.dart");
    let part = root.join("part.dart");
    let replacement = root.join("replacement.dart");
    let alias = root.join("alias.dart");
    let owner_source = "part 'part.dart'; class C { int _value = 0; }";
    let part_source = "part of 'owner.dart'; void write(C c) {}";
    std::fs::write(&owner, owner_source).unwrap();
    std::fs::write(&part, part_source).unwrap();
    std::fs::write(&replacement, "void unrelated() {}").unwrap();
    std::os::unix::fs::symlink(&part, &alias).unwrap();
    let owner_uri = uri(&owner);
    let alias_uri = uri(&alias);
    let mut state = state(root);
    state.open(&owner_uri, owner_source.into(), Some(1));
    state.open(&alias_uri, part_source.into(), Some(1));
    state.affected_open_uris(&alias_uri);
    state.change(
        &alias_uri,
        "part of 'owner.dart'; void write(C c) { c._value = 1; }".into(),
        Some(2),
    );
    std::fs::remove_file(&alias).unwrap();
    std::os::unix::fs::symlink(&replacement, &alias).unwrap();
    state.close(&alias_uri);
    assert_eq!(state.affected_open_uris(&alias_uri), vec![owner_uri]);
}

#[test]
fn close_before_pending_api_analysis_preserves_old_dependents() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let api = root.join("api.dart");
    let caller = root.join("main.dart");
    let caller_source = "import 'api.dart'; void main() { api(); }";
    std::fs::write(&api, "void api() {}").unwrap();
    std::fs::write(&caller, caller_source).unwrap();
    let api_uri = uri(&api);
    let caller_uri = uri(&caller);
    let mut state = state(root);
    state.open(&caller_uri, caller_source.into(), Some(1));
    state.open(&api_uri, "void api() {}".into(), Some(1));
    state.affected_open_uris(&api_uri);
    state.change(&api_uri, "int api() => 1;".into(), Some(2));
    assert!(state.semantic_snapshot.is_none());
    assert!(state.pending_semantic_affected[&api_uri].contains(&caller_uri));
    state.close(&api_uri);
    assert_eq!(state.affected_open_uris(&api_uri), vec![caller_uri.clone()]);
    assert!(state.analyze(&caller_uri).is_empty());
}
