use falcon_dart_parser::parse;
use falcon_syntax::cst::NodeKind;

#[test]
fn annotated_top_level_items_keep_metadata_inside_the_dispatched_node() {
    for (source, expected) in [
        ("@A library example;", NodeKind::LibraryDirective),
        ("@A part of example;", NodeKind::PartOfDirective),
        ("@A part 'example.dart';", NodeKind::PartDirective),
        ("@A import 'example.dart';", NodeKind::ImportDirective),
        ("@A export 'example.dart';", NodeKind::ExportDirective),
        ("@A class Example {}", NodeKind::ClassDecl),
        ("@A void example() {}", NodeKind::FunctionDecl),
        ("@A int example = 1;", NodeKind::TopLevelVarDecl),
        (
            "@A<int>.named() import 'example.dart';",
            NodeKind::ImportDirective,
        ),
    ] {
        let result = parse(source);
        assert!(result.errors.is_empty(), "{source}: {:?}", result.errors);
        assert!(result.tree.is_lossless(source));
        let root = result.tree.root(source);
        let item = root.children().next().unwrap();
        assert_eq!(item.kind(), expected, "{source}");
        assert_eq!(item.text(), source);
        let annotations: Vec<_> = item
            .children()
            .filter(|node| node.kind() == NodeKind::Annotation)
            .collect();
        assert_eq!(annotations.len(), 1, "{source}");
        assert_eq!(annotations[0].parent().unwrap().kind(), expected);
        assert_eq!(
            root.descendants()
                .filter(|node| node.kind() == NodeKind::Program)
                .count(),
            1,
            "{source}"
        );
    }
}

#[test]
fn annotations_are_dispatched_once_through_the_directive_preamble() {
    let source = "@A library example; @B part of example; @C import 'a.dart'; @D class C {}";
    let result = parse(source);
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert!(result.tree.is_lossless(source));
    let root = result.tree.root(source);
    assert_eq!(
        root.children().map(|node| node.kind()).collect::<Vec<_>>(),
        [
            NodeKind::LibraryDirective,
            NodeKind::PartOfDirective,
            NodeKind::ImportDirective,
            NodeKind::ClassDecl,
        ]
    );
    assert_eq!(
        root.descendants()
            .filter(|node| node.kind() == NodeKind::Annotation)
            .count(),
        4
    );
}

#[test]
fn metadata_does_not_reopen_the_directive_preamble_after_an_import() {
    for source in [
        "import 'a.dart'; @A library example;",
        "import 'a.dart'; @A part of example;",
    ] {
        let result = parse(source);
        assert!(result.tree.is_lossless(source), "{source}");
        let root = result.tree.root(source);
        assert_eq!(
            root.children().next().unwrap().kind(),
            NodeKind::ImportDirective,
            "{source}"
        );
        assert!(
            !root.descendants().any(|node| matches!(
                node.kind(),
                NodeKind::LibraryDirective | NodeKind::PartOfDirective
            )),
            "{source}"
        );
        assert_eq!(
            root.descendants()
                .filter(|node| node.kind() == NodeKind::Annotation)
                .count(),
            1,
            "{source}"
        );
    }
}

#[test]
fn long_metadata_run_is_owned_by_one_declaration() {
    const COUNT: usize = 2048;
    let source = format!("{}class Example {{}}", "@A ".repeat(COUNT));
    let result = parse(&source);
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert!(result.tree.is_lossless(&source));
    let root = result.tree.root(&source);
    assert_eq!(root.children().count(), 1);
    let declaration = root.children().next().unwrap();
    assert_eq!(declaration.kind(), NodeKind::ClassDecl);
    assert_eq!(
        declaration
            .children()
            .filter(|node| node.kind() == NodeKind::Annotation)
            .count(),
        COUNT
    );
}

#[test]
fn malformed_metadata_keeps_one_program_and_owned_annotations() {
    for source in [
        "@A",
        "@A @B",
        "@A } var after;",
        "@A part ; var after;",
        "@A import ; var after;",
        "class C { @A ) ; int after; }",
    ] {
        let result = parse(source);
        assert!(!result.errors.is_empty(), "{source}");
        assert!(result.tree.is_lossless(source), "{source}");
        let root = result.tree.root(source);
        assert_eq!(root.kind(), NodeKind::Program);
        assert_eq!(
            root.descendants()
                .filter(|node| node.kind() == NodeKind::Program)
                .count(),
            1,
            "{source}"
        );
        for annotation in root
            .descendants()
            .filter(|node| node.kind() == NodeKind::Annotation)
        {
            assert_ne!(
                annotation.parent().unwrap().kind(),
                NodeKind::Program,
                "{source}"
            );
        }
        if source.contains("after") {
            assert!(
                root.descendants()
                    .any(|node| node.kind() == NodeKind::VarDeclarator && node.text() == "after"),
                "{source}"
            );
        }
    }
}

#[test]
fn repeated_unclosed_annotation_arguments_recover_losslessly() {
    let source = "@A(".repeat(512);
    let result = parse(&source);
    assert!(!result.errors.is_empty());
    assert!(result.tree.is_lossless(&source));
    let root = result.tree.root(&source);
    assert_eq!(
        root.descendants()
            .filter(|node| node.kind() == NodeKind::Program)
            .count(),
        1
    );
}
