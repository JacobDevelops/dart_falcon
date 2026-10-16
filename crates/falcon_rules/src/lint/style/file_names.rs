//! Enforces `lowercase_with_underscores` for source file names.
//!
//! Adopted from the official `file_names` lint.
//! A file name is valid when it consists of lowercase letters, digits (not
//! first) and underscores, leading
//! underscores included. Names with an inner dot before `.dart` (generated
//! suffixes like `.g.dart`) are not checked.

use falcon_analyze::{AnalyzeContext, Rule, RulePreparation};
use falcon_diagnostics::Span as DiagSpan;
use falcon_syntax::Program;

#[derive(Default)]
pub struct FileNames;

impl Rule for FileNames {
    fn prepare<'a>(
        &self,
        _program: &'a Program,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let Some(file_name) = ctx.file_path.file_name().and_then(|name| name.to_str()) else {
            return RulePreparation::Diagnostics(Vec::new());
        };
        if valid_file_name(file_name) {
            return RulePreparation::Diagnostics(Vec::new());
        }
        RulePreparation::Diagnostics(vec![ctx.diagnostic(
            "file-names",
            "Name source files using lowercase_with_underscores.",
            DiagSpan { start: 0, end: 0 },
        )])
    }
}

/// The analyzer's `isValidDartFileName`.
fn valid_file_name(name: &str) -> bool {
    let Some(stem) = name.strip_suffix(".dart").filter(|stem| !stem.is_empty()) else {
        return true;
    };
    let bytes = stem.as_bytes();
    if bytes.len() > 2 && bytes[1..bytes.len() - 1].contains(&b'.') {
        return true;
    }
    bytes.iter().enumerate().all(|(i, &byte)| {
        byte.is_ascii_lowercase() || byte == b'_' || (i > 0 && byte.is_ascii_digit())
    })
}

#[cfg(test)]
mod tests {
    use super::valid_file_name;

    #[test]
    fn matches_the_analyzer() {
        for valid in [
            "_private_io.dart",
            "__.dart",
            "x.G-1.dart",
            "a1_b2.dart",
            "README",
        ] {
            assert!(valid_file_name(valid), "{valid}");
        }
        for invalid in ["Upper.dart", "kebab-case.dart", "1st.dart", ".hidden.dart"] {
            assert!(!valid_file_name(invalid), "{invalid}");
        }
    }
}
