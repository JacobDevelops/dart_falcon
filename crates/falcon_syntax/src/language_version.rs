//! Dart language versions and the features they gate.

use std::fmt;

/// A Dart language version (`major.minor`), ordered numerically.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LanguageVersion {
    pub major: u32,
    pub minor: u32,
}

/// Language features whose availability depends on the language version.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Feature {
    /// `C.new` and `C.named` constructor tear-offs.
    ConstructorTearoffs,
    /// A library directive without an explicit name.
    UnnamedLibraries,
    /// Records, patterns, class modifiers, sealed classes.
    Patterns,
    PrivateFieldPromotion,
    /// `_` declarations are non-binding wildcards.
    WildcardVariables,
    /// The formatter's tall style; trailing commas are managed by the formatter.
    TallStyle,
    PrimaryConstructors,
    PrivateNamedParameters,
}

impl Feature {
    /// The first language version enabling the feature.
    pub const fn since(self) -> LanguageVersion {
        let (major, minor) = match self {
            Self::ConstructorTearoffs => (2, 15),
            Self::UnnamedLibraries => (2, 19),
            Self::Patterns => (3, 0),
            Self::PrivateFieldPromotion => (3, 2),
            Self::WildcardVariables | Self::TallStyle => (3, 7),
            Self::PrivateNamedParameters => (3, 12),
            Self::PrimaryConstructors => (3, 13),
        };
        LanguageVersion::new(major, minor)
    }
}

impl LanguageVersion {
    /// The language version of the Dart SDK falcon models (Dart 3.13), which the
    /// analyzer applies to files outside any package and to SDK libraries.
    pub const CURRENT: Self = Self::new(3, 13);

    pub const fn new(major: u32, minor: u32) -> Self {
        Self { major, minor }
    }

    /// Whether code at this version may use `feature`.
    pub fn supports(self, feature: Feature) -> bool {
        self >= feature.since()
    }

    /// Parse `major.minor` (as in `package_config.json`'s `languageVersion`).
    pub fn parse(text: &str) -> Option<Self> {
        let (major, minor) = text.trim().split_once('.')?;
        let digits = |part: &str| {
            (!part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
                .then(|| part.parse().ok())
                .flatten()
        };
        Some(Self::new(digits(major)?, digits(minor)?))
    }

    /// The `// @dart = X.Y` override at the start of `source`, if any.
    ///
    /// Like the analyzer's scanner, only comments preceding the first token
    /// count, and the override must be a `//` line comment of exactly the form
    /// `@dart = X.Y`: only spaces (no tabs) may surround `@dart`, `=` and the
    /// version, and nothing but spaces may follow it on the line. Versions
    /// newer than [`Self::CURRENT`] are invalid and ignored.
    pub fn file_override(source: &str) -> Option<Self> {
        let mut rest = source.strip_prefix('\u{feff}').unwrap_or(source);
        if rest.starts_with("#!") {
            rest = rest.find('\n').map_or("", |end| &rest[end..]);
        }
        loop {
            rest = rest.trim_start();
            if let Some(comment) = rest.strip_prefix("//") {
                let end = comment.find(['\n', '\r']).unwrap_or(comment.len());
                if let Some(version) = Self::override_comment(&comment[..end]) {
                    return (version <= Self::CURRENT).then_some(version);
                }
                rest = &comment[end..];
            } else {
                rest = skip_block_comment(rest.strip_prefix("/*")?)?;
            }
        }
    }

    /// The version of a `//` comment body (the text after `//`, up to the line
    /// end) of the form `@dart = X.Y`, mirroring the analyzer scanner's
    /// `tokenizeLanguageVersionOrSingleLineComment`.
    fn override_comment(comment: &str) -> Option<Self> {
        let rest = comment.trim_start_matches(' ').strip_prefix("@dart")?;
        let rest = rest.trim_start_matches(' ').strip_prefix('=')?;
        let rest = rest.trim_start_matches(' ');
        let (major, rest) = leading_number(rest)?;
        let (minor, rest) = leading_number(rest.strip_prefix('.')?)?;
        rest.trim_start_matches(' ')
            .is_empty()
            .then_some(Self::new(major, minor))
    }
}

/// The decimal number at the start of `text` and the text after it.
fn leading_number(text: &str) -> Option<(u32, &str)> {
    let len = text.bytes().take_while(u8::is_ascii_digit).count();
    let digits = text.get(..len).filter(|digits| !digits.is_empty())?;
    Some((digits.parse().ok()?, &text[len..]))
}

/// The text after a (possibly nested) block comment whose opening `/*` has been
/// consumed; `None` when the comment is unterminated.
fn skip_block_comment(mut rest: &str) -> Option<&str> {
    let mut depth = 1usize;
    while depth > 0 {
        let close = rest.find("*/")?;
        let open = rest.find("/*").filter(|&open| open < close);
        if let Some(open) = open {
            depth += 1;
            rest = &rest[open + 2..];
        } else {
            depth -= 1;
            rest = &rest[close + 2..];
        }
    }
    Some(rest)
}

impl fmt::Display for LanguageVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_orders_versions() {
        assert_eq!(
            LanguageVersion::parse("3.10"),
            Some(LanguageVersion::new(3, 10))
        );
        assert!(LanguageVersion::new(3, 10) > LanguageVersion::new(3, 9));
        assert_eq!(LanguageVersion::parse("3"), None);
        assert_eq!(LanguageVersion::parse("3.x"), None);
    }

    #[test]
    fn feature_gates() {
        assert!(LanguageVersion::new(3, 7).supports(Feature::WildcardVariables));
        assert!(!LanguageVersion::new(3, 6).supports(Feature::WildcardVariables));
        assert!(LanguageVersion::CURRENT.supports(Feature::PrimaryConstructors));
    }

    #[test]
    fn reads_leading_override_only() {
        let version = |source: &str| LanguageVersion::file_override(source);
        assert_eq!(
            version("// Generated code.\n// @dart = 3.3\nimport 'a.dart';"),
            Some(LanguageVersion::new(3, 3))
        );
        assert_eq!(version("//@dart=2.19\n"), Some(LanguageVersion::new(2, 19)));
        assert_eq!(
            version("/* header /* nested */ */\n// @dart = 3.1\n"),
            Some(LanguageVersion::new(3, 1))
        );
        assert_eq!(
            version("#!/usr/bin/env dart\n// @dart=3.0\n"),
            Some(LanguageVersion::new(3, 0))
        );
        assert_eq!(version("import 'a.dart';\n// @dart = 3.3\n"), None);
        assert_eq!(version("// @dart = 3.3 extra\n"), None);
        assert_eq!(version("/// @dart = 3.3\n"), None);
        assert_eq!(version("// @dart = 9.0\n"), None);
        assert_eq!(
            version("/* c */\n// @dart = 2.19\n"),
            Some(LanguageVersion::new(2, 19))
        );
        assert_eq!(
            version("// @dart = 2.19  \r\n"),
            Some(LanguageVersion::new(2, 19))
        );
    }

    #[test]
    fn rejects_malformed_markers() {
        let version = |source: &str| LanguageVersion::file_override(source);
        assert_eq!(version("// @dart = 2\n"), None);
        assert_eq!(version("// @dart = 2.19.0\n"), None);
        assert_eq!(version("// @dart = 2.19 // why\n"), None);
        assert_eq!(version("// @dart\t= 2.19\n"), None);
        assert_eq!(version("// @dart = 2.19\t\n"), None);
        assert_eq!(version("// @dart = .19\n"), None);
    }
}
