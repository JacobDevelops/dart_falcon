//! How `dart:core` names are reachable in a file.
//!
//! `dart:core` is imported implicitly only while the file has no explicit
//! `import 'dart:core'`; once it imports `dart:core` with a prefix only,
//! unprefixed core names (including `dynamic`) are no longer in scope.

use falcon_syntax::Program;
use falcon_syntax::ast::{DartType, Identifier, ImportDirective};

pub(crate) struct CoreImports<'p> {
    imports: Vec<&'p ImportDirective>,
}

impl<'p> CoreImports<'p> {
    pub(crate) fn new(program: &'p Program) -> Self {
        let imports = program
            .imports
            .iter()
            .filter(|import| import.uri.value == "dart:core")
            .collect();
        Self { imports }
    }

    /// Unprefixed core names are in scope: the implicit `dart:core` import, or
    /// an explicit unprefixed one.
    pub(crate) fn unprefixed(&self) -> bool {
        self.imports.is_empty() || self.imports.iter().any(|import| import.as_name.is_none())
    }

    /// Whether `prefix` is the prefix of an `import 'dart:core' as prefix;`.
    pub(crate) fn is_prefix(&self, prefix: &str) -> bool {
        self.imports.iter().any(|import| {
            import
                .as_name
                .as_ref()
                .is_some_and(|name| name.name == prefix)
        })
    }

    /// Whether `segments` (`[name]` or `[prefix, name]`) reach the core
    /// declaration `name` through an import (ignoring local shadowing).
    pub(crate) fn reaches(&self, segments: &[Identifier], name: &str) -> bool {
        match segments {
            [only] => only.name == name && self.unprefixed(),
            [prefix, last] => last.name == name && self.is_prefix(&prefix.name),
            _ => false,
        }
    }

    /// Whether the written type annotation `ty` denotes `dynamic`.
    pub(crate) fn is_dynamic(&self, ty: &DartType) -> bool {
        match ty {
            DartType::Dynamic { .. } => self.unprefixed(),
            DartType::Named(named) => {
                named.type_args.is_empty() && self.reaches(&named.segments, "dynamic")
            }
            _ => false,
        }
    }
}
