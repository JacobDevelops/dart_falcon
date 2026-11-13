use super::*;
use std::path::Path;
use std::sync::Arc;

#[test]
fn import_namespaces_follow_reexports_combinators_and_prefixes() {
    let inputs = [
        (
            "/project/main.dart",
            "import 'package:flutter/foundation.dart'; import 'package:meta/meta.dart' show immutable; import 'package:flutter/foundation.dart' as f hide protected;",
        ),
        (
            "/project/flutter/foundation.dart",
            "export 'package:meta/meta.dart';",
        ),
        (
            "/project/meta/meta.dart",
            "const immutable = 1; const protected = 2; const visibleForTesting = 3;",
        ),
    ];
    let parsed: Vec<_> = inputs
        .iter()
        .map(|(_, source)| {
            let (parsed, errors) = crate::ParsedFile::parse_for_package(*source, None);
            assert!(errors.is_empty(), "{errors:?}");
            Arc::new(parsed)
        })
        .collect();
    let sources: Vec<_> = inputs
        .iter()
        .zip(&parsed)
        .map(|((path, _), parsed)| IdentitySource {
            path: Path::new(path),
            parsed,
            has_parse_errors: false,
        })
        .collect();
    let packages = [
        PackageIdentity {
            name: "flutter".into(),
            lib_root: "/project/flutter".into(),
        },
        PackageIdentity {
            name: "meta".into(),
            lib_root: "/project/meta".into(),
        },
    ];
    let index = IdentityIndex::from_project_files(&sources, &packages);
    let library = index.library(index.file(Path::new(inputs[0].0)).unwrap());
    let immutable = index
        .resolve_imported_member(library, "immutable".into())
        .unwrap();
    for import in [0, 1] {
        assert_eq!(
            index.import_provides(library, import, "immutable".into(), &immutable),
            Some(true)
        );
    }
    let visible = index
        .resolve_imported_member(library, "visibleForTesting".into())
        .unwrap();
    assert_eq!(
        index.import_provides(library, 0, "visibleForTesting".into(), &visible),
        Some(true)
    );
    assert_eq!(
        index.import_provides(library, 1, "visibleForTesting".into(), &visible),
        Some(false)
    );
    assert_eq!(
        index.import_provides(
            library,
            2,
            QualifiedName::prefixed("f", "immutable"),
            &immutable
        ),
        Some(true)
    );
    let protected = index
        .resolve_imported_member(library, "protected".into())
        .unwrap();
    assert_eq!(
        index.import_provides(
            library,
            2,
            QualifiedName::prefixed("f", "protected"),
            &protected
        ),
        Some(false)
    );
    assert_eq!(
        index.import_provides(library, 2, "immutable".into(), &immutable),
        Some(false)
    );
}
