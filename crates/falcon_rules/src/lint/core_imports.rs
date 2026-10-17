//! How `dart:core` names are reachable in a file.
//!
//! `dart:core` is imported implicitly only while the file has no explicit
//! `import 'dart:core'`; once it imports `dart:core` with a prefix only,
//! unprefixed core names (including `dynamic`) are no longer in scope.

use falcon_syntax::cst::{DartType, Identifier, ImportDirective, Program};

pub(crate) struct CoreImports<'p> {
    imports: Vec<ImportDirective<'p>>,
}

impl<'p> CoreImports<'p> {
    pub(crate) fn new(program: Program<'p>) -> Self {
        let imports = program
            .imports()
            .filter(|import| import.uri().value() == "dart:core")
            .collect();
        Self { imports }
    }

    /// Unprefixed core names are in scope: the implicit `dart:core` import, or
    /// an explicit unprefixed one.
    pub(crate) fn unprefixed(&self) -> bool {
        self.imports.is_empty() || self.imports.iter().any(|import| import.as_name().is_none())
    }

    /// Whether `prefix` is the prefix of an `import 'dart:core' as prefix;`.
    pub(crate) fn is_prefix(&self, prefix: &str) -> bool {
        self.imports
            .iter()
            .any(|import| import.as_name().is_some_and(|name| name.name() == prefix))
    }

    /// Whether `segments` (`[name]` or `[prefix, name]`) reach the core
    /// declaration `name` through an import (ignoring local shadowing).
    pub(crate) fn reaches<'a>(
        &self,
        mut segments: impl Iterator<Item = Identifier<'a>>,
        name: &str,
    ) -> bool {
        let Some(first) = segments.next() else {
            return false;
        };
        match (segments.next(), segments.next()) {
            (None, None) => first.name() == name && self.unprefixed(),
            (Some(last), None) => last.name() == name && self.is_prefix(first.name()),
            _ => false,
        }
    }

    /// Whether the written type annotation `ty` denotes `dynamic`.
    pub(crate) fn is_dynamic(&self, ty: DartType<'_>) -> bool {
        match ty {
            DartType::Dynamic(_) => self.unprefixed(),
            DartType::Named(named) => {
                named.type_args().is_empty() && self.reaches(named.segments(), "dynamic")
            }
            _ => false,
        }
    }
}
