//! Require the targets of conditional import and export URIs to exist.
//!
//! Adopted from the official `conditional_uri_does_not_exist` lint. Every
//! configurable URI (`if (dart.library.io) 'io_impl.dart'`) on an `import` or
//! `export` is resolved as the analyzer resolves it, and reported when it
//! names no existing source:
//!
//! - A relative URI resolves against the URI of the containing file: its
//!   `package:` URI inside a package's `lib/` directory, so climbing out of
//!   `lib/` names another package, else its `file:` URI.
//! - A `package:` URI resolves through the package configuration. Without one,
//!   every `package:` URI is reported. A query is ignored; a fragment, an
//!   unknown package, or a missing path segment is reported.
//! - A `file:` URI needs an absolute path; its authority is ignored. A query or
//!   fragment (including on a relative URI resolved to `file:`) names no file.
//! - A `dart:` URI must name a library of the SDK (with its embedder); without
//!   a located SDK only invalid paths, queries and fragments are reported.
//! - Any other scheme is reported.
//!
//! Invalid schemes and invalid percent escapes still name missing sources;
//! invalid percent escapes are interpreted as literal percent signs.
//! The default (unconditional) URI is not checked by this rule.
//! Package and SDK metadata come from the run's shared analysis environment;
//! callers must attach it to resolve configured package sources.

use std::path::{Path, PathBuf};

use falcon_analyze::{AnalyzeContext, PackageConfig, Rule, RulePreparation, percent_decode};
use falcon_syntax::cst::{Program, StringLitNode};

use crate::lint::correctness::always_use_package_imports::uri_scheme;
use crate::lint::string_lit::{decode_escapes, decoded_text, literal_parts};

#[derive(Default)]
pub struct ConditionalUriDoesNotExist;

const NAME: &str = "conditional-uri-does-not-exist";

impl Rule for ConditionalUriDoesNotExist {
    fn prepare<'a>(
        &self,
        program: Program<'a>,
        ctx: &'a AnalyzeContext<'a>,
    ) -> RulePreparation<'a> {
        let mut configurations = program
            .imports()
            .flat_map(|import| import.configurable_uris())
            .chain(
                program
                    .exports()
                    .flat_map(|export| export.configurable_uris()),
            )
            .peekable();
        if configurations.peek().is_none() {
            return RulePreparation::Diagnostics(Vec::new());
        }
        let config = ctx
            .environment
            .and_then(|environment| environment.package_config(ctx.file_path));
        let resolver = Resolver {
            file: ctx.file_path,
            config,
            sdk_library: &|uri| {
                let sdk = ctx.environment?.sdk(ctx.file_path)?;
                Some(sdk.library(uri).is_some())
            },
        };
        RulePreparation::Diagnostics(
            configurations
                .filter_map(|configuration| {
                    let uri = configuration.uri();
                    let value = uri_value(uri);
                    (resolver.exists(&value) == Some(false)).then(|| {
                        ctx.diagnostic(
                            NAME,
                            format!("The target of the conditional URI '{value}' doesn't exist."),
                            uri.span(),
                        )
                    })
                })
                .collect(),
        )
    }
}

struct Resolver<'a> {
    file: &'a Path,
    config: Option<&'a PackageConfig>,
    /// Whether the SDK defines a `dart:` library; `None` without an SDK.
    sdk_library: &'a dyn Fn(&str) -> Option<bool>,
}

impl Resolver<'_> {
    /// Whether `uri` names an existing source; `None` when unknown.
    fn exists(&self, uri: &str) -> Option<bool> {
        let scheme = match uri_scheme(uri) {
            Some(scheme) => scheme.to_ascii_lowercase(),
            None if uri.contains('%') => {
                let mut normalized = String::with_capacity(uri.len());
                for (index, ch) in uri.char_indices() {
                    if ch == '%'
                        && !uri
                            .as_bytes()
                            .get(index + 1..index + 3)
                            .is_some_and(|hex| hex.iter().all(u8::is_ascii_hexdigit))
                    {
                        normalized.push_str("%25");
                    } else {
                        normalized.push(ch);
                    }
                }
                return if normalized == uri {
                    Some(false)
                } else {
                    self.exists(&normalized)
                };
            }
            None => return Some(false),
        };
        let (rest, fragment) = match uri.split_once('#') {
            Some((rest, _)) => (rest, true),
            None => (uri, false),
        };
        let (rest, query) = match rest.split_once('?') {
            Some((rest, _)) => (rest, true),
            None => (rest, false),
        };
        match scheme.as_str() {
            "" => {
                if uri.is_empty() {
                    // The containing library itself.
                    return Some(true);
                }
                match self.base() {
                    Some(base) => {
                        let path = merge(&base, rest);
                        self.package_exists(&remove_dot_segments(&path), fragment)
                    }
                    None => {
                        if query || fragment {
                            return Some(false);
                        }
                        let directory = self.file.parent().unwrap_or(Path::new(""));
                        Some(directory.join(percent_decode(rest)?).is_file())
                    }
                }
            }
            "package" => {
                let path = &rest["package:".len()..];
                self.package_exists(&remove_dot_segments(path), fragment)
            }
            "dart" => {
                let library = &rest["dart:".len()..];
                if query || fragment || library.contains('/') {
                    return Some(false);
                }
                (self.sdk_library)(&format!("dart:{library}"))
            }
            "file" => {
                if query || fragment {
                    return Some(false);
                }
                let path = &rest["file:".len()..];
                let path = path.strip_prefix("//").map_or(path, |authority| {
                    authority.find('/').map_or("", |slash| &authority[slash..])
                });
                let path = PathBuf::from(percent_decode(path)?);
                Some(path.is_absolute() && path.is_file())
            }
            _ => Some(false),
        }
    }

    /// The `package:` URI path (`name/dir/file.dart`) of the containing file
    /// when it lies in a package's `lib/` directory.
    fn base(&self) -> Option<String> {
        let package = self.config?.package_library_containing(self.file)?;
        let (name, lib_root) = (package.name.as_str(), package.lib_root.as_path());
        let file = falcon_analyze::canonical_or_lexical(self.file);
        let relative = file.strip_prefix(lib_root).ok()?;
        let mut path = name.to_string();
        for component in relative.components() {
            path.push('/');
            path.push_str(&component.as_os_str().to_string_lossy());
        }
        Some(path)
    }

    /// Whether `package:<path>` names an existing file.
    fn package_exists(&self, path: &str, fragment: bool) -> Option<bool> {
        let Some(config) = self.config else {
            return Some(false);
        };
        let (_, rest) = path.split_once('/').unwrap_or((path, ""));
        if fragment || rest.is_empty() {
            return Some(false);
        }
        let target = config.resolve_package_uri(&format!("package:{path}"));
        Some(target.is_some_and(|target| target.is_file()))
    }
}

/// Decode each adjacent literal independently: one raw segment does not make
/// its non-raw neighbours raw.
pub(crate) fn uri_value(uri: StringLitNode<'_>) -> String {
    if !uri.is_adjacent() {
        if let Some(literal) = uri.delimiter()
            && literal.triple
        {
            return decoded_text(&literal, &literal_parts(&literal));
        }
        return if uri.has_raw_segment() {
            uri.value().into_owned()
        } else {
            decode_escapes(&uri.value()).into_owned()
        };
    }
    let mut value = String::with_capacity(
        uri.segments()
            .map(|(literal, _)| literal.content.len())
            .sum(),
    );
    for (literal, _) in uri.segments() {
        if literal.triple {
            value.push_str(&decoded_text(&literal, &literal_parts(&literal)));
        } else if literal.is_raw {
            value.push_str(literal.content);
        } else {
            value.push_str(&decode_escapes(literal.content));
        }
    }
    value
}

/// RFC 3986 merge of a relative reference path into the base URI path.
fn merge(base: &str, reference: &str) -> String {
    if reference.is_empty() {
        return base.to_string();
    }
    if reference.starts_with('/') {
        return reference.to_string();
    }
    let directory = base.rfind('/').map_or("", |slash| &base[..=slash]);
    format!("{directory}{reference}")
}

/// RFC 3986 `remove_dot_segments`, as `Uri.resolve` applies it to URIs with a
/// scheme: `..` never climbs above the first segment.
fn remove_dot_segments(path: &str) -> String {
    let mut output: Vec<&str> = Vec::new();
    let segments: Vec<&str> = path.split('/').collect();
    let last = segments.len() - 1;
    for (index, segment) in segments.iter().enumerate() {
        match *segment {
            "." => {
                if index == last {
                    output.push("");
                }
            }
            ".." => {
                output.pop();
                if index == last {
                    output.push("");
                }
            }
            segment => output.push(segment),
        }
    }
    output.join("/")
}

#[cfg(test)]
mod tests {
    use super::{ConditionalUriDoesNotExist, Resolver, merge, remove_dot_segments};
    use falcon_analyze::RuleAnalysis;

    #[test]
    fn project_fixture_matches_expectations() {
        crate::cross_file::assert_project_fixture(
            "conditional-uri-does-not-exist",
            &ConditionalUriDoesNotExist,
        );
    }

    #[test]
    fn configured_uris_match_analyzer_resolution() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/corpus/conditional-uri-does-not-exist/project");
        let config = falcon_analyze::PackageConfig::parse(
            &root.join(".dart_tool/package_config.json"),
            r#"{"configVersion":2,"packages":[{"name":"app","rootUri":"../","packageUri":"lib/","languageVersion":"3.13"}]}"#,
        ).unwrap();
        let file = root.join("lib/bad_package_uris.dart");
        let resolver = Resolver {
            file: &file,
            config: Some(&config),
            sdk_library: &|_| None,
        };
        assert_eq!(resolver.exists("package:other/missing.dart"), Some(false));
        assert_eq!(
            resolver.exists("package:app/src/present.dart?query"),
            Some(true)
        );
        assert_eq!(
            resolver.exists("package:app/src/present.dart#fragment"),
            Some(false)
        );
        assert_eq!(resolver.exists("../../lib/src/present.dart"), Some(false));
        // The importing fixture is directly in lib/, not in a nested directory.
        assert_eq!(resolver.exists("../src/present.dart"), Some(false));
        assert_eq!(resolver.exists("src/present.dart"), Some(true));
        let target = root.join("lib/src/present.dart");
        assert_eq!(
            resolver.exists(&format!("file:{}", target.display())),
            Some(true)
        );
        assert_eq!(
            resolver.exists(&format!("file://remote{}", target.display())),
            Some(true)
        );
        assert_eq!(resolver.exists("file:lib/src/present.dart"), Some(false));
    }

    #[test]
    fn package_uris_without_configuration_are_missing() {
        let resolver = Resolver {
            file: std::path::Path::new("lib/main.dart"),
            config: None,
            sdk_library: &|_| None,
        };
        assert_eq!(resolver.exists("package:app/src/present.dart"), Some(false));
        assert_eq!(resolver.exists("package:other/missing.dart"), Some(false));
    }

    #[test]
    fn shared_environment_resolves_configured_package_queries() {
        use falcon_analyze::{AnalysisEnvironment, AnalyzeContext};
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/corpus/conditional-uri-does-not-exist/project");
        let file = root.join("lib/good_package_uris.dart");
        let source = "import 'src/present.dart'\n\
            if (dart.library.io) 'package:configured_app/src/present.dart?query'\n\
            if (dart.library.html) '?query'\n\
            if (dart.library.js_interop) 'package:configured_app/src/present.dart#fragment';";
        let falcon_dart_parser::ParseResult { tree, errors } =
            falcon_dart_parser::parser::parse(source);
        assert!(errors.is_empty(), "{errors:?}");
        let parsed = std::sync::Arc::new(falcon_analyze::ParsedFile {
            source: source.into(),
            tree,
        });
        let environment =
            AnalysisEnvironment::discover_with_sdk_locator([file.as_path()], |_| None);
        let config = falcon_config::FalconConfig::default();
        let context =
            AnalyzeContext::new(&file, &config, &parsed).with_environment(Some(&environment));
        let diagnostics = ConditionalUriDoesNotExist.analyze(parsed.program(), &context);
        assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
        assert_eq!(
            &source[diagnostics[0].span.start..diagnostics[0].span.end],
            "'package:configured_app/src/present.dart#fragment'"
        );
    }

    #[test]
    fn relative_references_resolve_like_uri_resolve() {
        let resolve = |base, reference| remove_dot_segments(&merge(base, reference));
        assert_eq!(resolve("app/src/a.dart", ""), "app/src/a.dart");
        assert_eq!(resolve("app/src/a.dart", "b.dart"), "app/src/b.dart");
        assert_eq!(resolve("app/src/a.dart", "../b.dart"), "app/b.dart");
        assert_eq!(resolve("app/a.dart", "../../lib/b.dart"), "lib/b.dart");
        assert_eq!(resolve("app/a.dart", "./b.dart"), "app/b.dart");
        assert_eq!(remove_dot_segments("app/../app/b.dart"), "app/b.dart");
    }
}
