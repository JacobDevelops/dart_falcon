//! Where a file sits inside its enclosing pub package.

use std::path::Path;

/// Whether `path` lies under the top-level `lib/` directory of its enclosing
/// package (the directory holding the nearest `pubspec.yaml`) — the analyzer's
/// `RuleContext.isInLibDir`.
pub(crate) fn is_in_lib_dir(path: &Path) -> bool {
    let Some(pubspec) = crate::cross_file::enclosing_pubspec(path) else {
        return false;
    };
    let Some(root) = pubspec.parent() else {
        return false;
    };
    falcon_analyze::canonical_or_lexical(path)
        .strip_prefix(root)
        .ok()
        .and_then(|relative| relative.components().next())
        .is_some_and(|first| first.as_os_str() == "lib")
}
