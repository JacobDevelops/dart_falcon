//! The rule table supplies constructors, execution identities, and configuration metadata.

use std::collections::HashMap;
use std::sync::LazyLock;

use falcon_analyze::{CrossFileRule, Rule};
use falcon_config::{FalconConfig, RulePlainConfiguration, Rules};

use crate::{cross_file, lint};

macro_rules! declare_rules {
    ($group:ident; $($module:ident::$rule:ident => ($name:literal, $kind:ident, $domain:expr, $recommended:literal, $source:expr $(, options = $options:ty)?);)*) => {
        $crate::declare_rules! {
            $($module::$rule => ($name, $kind, $group, $domain, $recommended, $source $(, options = $options)?);)*
        }
    };
    ($($module:ident::$rule:ident => ($name:literal, $kind:ident, $group:ident, $recommended:literal, $source:expr $(, options = $options:ty)?);)*) => {
        $crate::declare_rules! {
            $($module::$rule => ($name, $kind, $group, None, $recommended, $source $(, options = $options)?);)*
        }
    };
    ($($module:ident::$rule:ident => ($name:literal, $kind:ident, $group:ident, $domain:expr, $recommended:literal, $source:expr $(, options = $options:ty)?);)*) => {
        $(
            pub mod $module;
            impl $module::$rule {
                pub const NAME: &'static str = $name;
            }
        )*
        pub(crate) const METADATA: &[$crate::meta::RuleMeta] = &[
            $( $crate::declare_rules!(@meta $kind, $module::$rule, $group, $domain, $recommended, $source).with_options($crate::declare_rules!(@options $($options)?)), )*
        ];
    };
    (@options $options:ty) => {
        Some($crate::lint::options::OptionDefinition::new::<$options>())
    };
    (@options) => { None };
    (@meta cross_file, $rule:path, $group:ident, $domain:expr, $recommended:literal, $source:expr) => {
        $crate::meta::RuleMeta::cross_file::<$rule>(<$rule>::NAME, $crate::meta::RuleGroup::$group, $recommended, $source)
    };
    (@meta resolved_cross_file, $rule:path, $group:ident, $domain:expr, $recommended:literal, $source:expr) => {
        $crate::meta::RuleMeta::resolved_cross_file::<$rule>(<$rule>::NAME, $crate::meta::RuleGroup::$group, $recommended, $source)
    };
    (@meta $kind:ident, $rule:path, $group:ident, $domain:expr, $recommended:literal, $source:expr) => {
        $crate::meta::RuleMeta::$kind::<$rule>(<$rule>::NAME, $crate::meta::RuleGroup::$group, $domain, $recommended, $source)
    };
}
pub(crate) use declare_rules;

macro_rules! metadata_enum {
    ($name:ident { $($variant:ident => $label:literal),+ $(,)? }) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum $name {
            $($variant),+
        }

        impl $name {
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $label),+
                }
            }

            pub const fn names(self) -> &'static [&'static str] {
                match self {
                    $(Self::$variant => &[$label]),+
                }
            }
        }
    };
}

metadata_enum!(RuleGroup {
    A11y => "a11y",
    Complexity => "complexity",
    Correctness => "correctness",
    Performance => "performance",
    Security => "security",
    Style => "style",
    Suspicious => "suspicious",
});

metadata_enum!(Domain {
    Flutter => "flutter",
    Test => "test",
    Bloc => "bloc",
    Riverpod => "riverpod",
    Provider => "provider",
    FlutterHooks => "flutter_hooks",
    Equatable => "equatable",
});

#[derive(Clone, Copy)]
pub enum RuleKind {
    File {
        make: fn() -> Box<dyn Rule>,
        needs_resolution: bool,
    },
    CrossFile {
        make: fn() -> Box<dyn CrossFileRule>,
        needs_resolution: bool,
    },
}

/// Upstream provenance of a rule: which linter it was ported from (with the
/// upstream rule id, which occasionally differs from falcon's), or whether it is
/// original to falcon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuleSource {
    /// Ported from dart_code_linter; carries the upstream rule/command id.
    DartCodeLinter(&'static str),
    /// Ported from pyramid_lint; carries the upstream rule id.
    PyramidLint(&'static str),
    /// Adopted from the official `package:lints` / `package:flutter_lints`
    /// rule sets; carries the official (snake_case) lint id.
    Lints(&'static str),
    /// Original to falcon — no upstream equivalent.
    Falcon,
}

/// A rule's execution identity, constructor, and configuration metadata.
#[derive(Clone, Copy)]
pub struct RuleMeta {
    pub name: &'static str,
    pub group: RuleGroup,
    pub domain: Option<Domain>,
    pub recommended: bool,
    pub kind: RuleKind,
    /// Upstream provenance (biome-style): which linter the rule was ported from,
    /// or `Falcon` for original rules.
    pub source: RuleSource,
    pub(crate) options: Option<crate::lint::options::OptionDefinition>,
}

impl RuleMeta {
    pub(crate) const fn file<T: Rule + Default + 'static>(
        name: &'static str,
        group: RuleGroup,
        domain: Option<Domain>,
        recommended: bool,
        source: RuleSource,
    ) -> Self {
        Self {
            name,
            group,
            domain,
            recommended,
            source,
            options: None,
            kind: RuleKind::File {
                make: make_rule::<T>,
                needs_resolution: false,
            },
        }
    }

    pub(crate) const fn resolved<T: Rule + Default + 'static>(
        name: &'static str,
        group: RuleGroup,
        domain: Option<Domain>,
        recommended: bool,
        source: RuleSource,
    ) -> Self {
        let mut meta = Self::file::<T>(name, group, domain, recommended, source);
        meta.kind = RuleKind::File {
            make: make_rule::<T>,
            needs_resolution: true,
        };
        meta
    }

    pub(crate) const fn cross_file<T: CrossFileRule + Default + 'static>(
        name: &'static str,
        group: RuleGroup,
        recommended: bool,
        source: RuleSource,
    ) -> Self {
        Self {
            name,
            group,
            domain: None,
            recommended,
            source,
            options: None,
            kind: RuleKind::CrossFile {
                make: make_cross_file_rule::<T>,
                needs_resolution: false,
            },
        }
    }

    pub(crate) const fn resolved_cross_file<T: CrossFileRule + Default + 'static>(
        name: &'static str,
        group: RuleGroup,
        recommended: bool,
        source: RuleSource,
    ) -> Self {
        let mut meta = Self::cross_file::<T>(name, group, recommended, source);
        meta.kind = RuleKind::CrossFile {
            make: make_cross_file_rule::<T>,
            needs_resolution: true,
        };
        meta
    }
    pub(crate) const fn with_options(
        mut self,
        options: Option<crate::lint::options::OptionDefinition>,
    ) -> Self {
        self.options = options;
        self
    }

    pub fn domain_names(&self) -> &'static [&'static str] {
        self.domain.map_or(&[], Domain::names)
    }

    pub const fn is_cross_file(&self) -> bool {
        matches!(self.kind, RuleKind::CrossFile { .. })
    }
}

fn make_rule<T: Rule + Default + 'static>() -> Box<dyn Rule> {
    Box::new(T::default())
}

fn make_cross_file_rule<T: CrossFileRule + Default + 'static>() -> Box<dyn CrossFileRule> {
    Box::new(T::default())
}

const GROUP_METADATA: &[&[RuleMeta]] = &[
    lint::a11y::METADATA,
    lint::complexity::METADATA,
    lint::correctness::METADATA,
    lint::performance::METADATA,
    lint::security::METADATA,
    lint::style::METADATA,
    lint::suspicious::METADATA,
    cross_file::METADATA,
];

const RULE_COUNT: usize = {
    let mut count = 0;
    let mut group = 0;
    while group < GROUP_METADATA.len() {
        count += GROUP_METADATA[group].len();
        group += 1;
    }
    count
};

const fn combine_metadata() -> [RuleMeta; RULE_COUNT] {
    let mut metadata = [GROUP_METADATA[0][0]; RULE_COUNT];
    let mut output = 0;
    let mut group = 0;
    while group < GROUP_METADATA.len() {
        let mut rule = 0;
        while rule < GROUP_METADATA[group].len() {
            metadata[output] = GROUP_METADATA[group][rule];
            output += 1;
            rule += 1;
        }
        group += 1;
    }
    metadata
}

/// The combined constructor table emitted by the per-group rule declarations.
pub const RULE_METADATA: &[RuleMeta] = &combine_metadata();

/// Legacy rule ids mapped to their canonical id. Covers the pre-1.0
/// `snake_case` → `kebab-case` renames and the removed twin rules (whose
/// pyramid_lint-derived variants were merged into the surviving dart_code_linter
/// rule). Keeps old `falcon.json` rule keys and old `// falcon-ignore
/// lint/<group>/<id>` suppression paths resolving after the id normalization.
pub const RULE_ALIASES: &[(&str, &str)] = &[
    ("prefer-boolean-prefixes", "boolean-prefixes"),
    // ── snake_case → kebab-case renames ──
    (
        "avoid_inverted_boolean_expressions",
        "avoid-inverted-boolean-expressions",
    ),
    ("avoid_nested_if", "avoid-nested-if"),
    ("max_lines_for_file", "max-lines-for-file"),
    ("max_lines_for_function", "max-lines-for-function"),
    ("max_parameters_for_function", "max-parameters-for-function"),
    ("max_switch_cases", "max-switch-cases"),
    ("cyclomatic_complexity", "cyclomatic-complexity"),
    ("maximum_nesting_level", "maximum-nesting-level"),
    ("prefer_iterable_any", "prefer-iterable-any"),
    ("prefer_iterable_every", "prefer-iterable-every"),
    (
        "avoid_mutable_global_variables",
        "avoid-mutable-global-variables",
    ),
    (
        "correct_order_for_super_dispose",
        "correct-order-for-super-dispose",
    ),
    ("unnecessary_flutter_imports", "unnecessary-flutter-imports"),
    (
        "unnecessary_nullable_return_type",
        "unnecessary-nullable-return-type",
    ),
    (
        "use_once_constructors_once_provider",
        "use-once-constructors-once-provider",
    ),
    ("proper_controller_dispose", "proper-controller-dispose"),
    (
        "proper_expanded_and_flexible",
        "proper-expanded-and-flexible",
    ),
    ("proper_from_environment", "proper-from-environment"),
    ("proper_super_init_state", "proper-super-init-state"),
    (
        "prefer_declaring_const_constructor",
        "prefer-declaring-const-constructor",
    ),
    (
        "avoid_abbreviations_in_doc_comments",
        "avoid-abbreviations-in-doc-comments",
    ),
    (
        "avoid_positional_fields_in_records",
        "avoid-positional-fields-in-records",
    ),
    ("boolean_prefixes", "boolean-prefixes"),
    ("class_members_ordering", "class-members-ordering"),
    (
        "prefer_dedicated_media_query_methods",
        "prefer-dedicated-media-query-methods",
    ),
    (
        "prefer_underscore_for_unused_callback_parameters",
        "prefer-underscore-for-unused-callback-parameters",
    ),
    (
        "use_spacer_as_expanded_child",
        "use-spacer-as-expanded-child",
    ),
    (
        "avoid_single_child_column_or_row",
        "avoid-single-child-column-or-row",
    ),
    ("prefer_async_callback", "prefer-async-callback"),
    (
        "avoid_redundant_pattern_field_names",
        "avoid-redundant-pattern-field-names",
    ),
    ("no_self_comparisons", "no-self-comparisons"),
    ("no_duplicate_case_values", "no-duplicate-case-values"),
    // ── unified twins: pyramid_lint variant → surviving canonical rule ──
    ("avoid_unused_parameters", "avoid-unused-parameters"),
    ("no_magic_number", "no-magic-number"),
    ("avoid_empty_blocks", "no-empty-block"),
    ("no_empty_block", "no-empty-block"),
    (
        "avoid-unnecessary-conditionals",
        "avoid-bool-literals-in-conditional-expressions",
    ),
    ("avoid-self-compare", "no-self-comparisons"),
];

/// Resolve a possibly-legacy rule id to its canonical id. Returns the input
/// unchanged when it is already canonical (or simply unknown).
pub fn canonical_rule_name(name: &str) -> &str {
    RULE_ALIASES
        .iter()
        .find(|(old, _)| *old == name)
        .map(|&(_, canonical)| canonical)
        .unwrap_or(name)
}

/// Look up metadata for a rule by its registered name, resolving legacy aliases
/// first so an old id still finds its canonical rule.
pub fn meta_for(name: &str) -> Option<&'static RuleMeta> {
    let canonical = canonical_rule_name(name);
    static BY_NAME: LazyLock<HashMap<&'static str, &'static RuleMeta>> =
        LazyLock::new(|| RULE_METADATA.iter().map(|meta| (meta.name, meta)).collect());
    BY_NAME.get(canonical).copied()
}

/// Suppression-path validation hook: maps a (possibly-legacy) rule name to its
/// `(canonical_name, group, is_cross_file)` so `falcon_analyze` can validate a
/// `// falcon-ignore` path — and record the *canonical* id, so a suppression
/// written with an old id still matches the diagnostic's canonical rule — without
/// depending on this crate. Matches [`falcon_analyze::RuleLookup`].
pub fn suppression_lookup(name: &str) -> Option<(&'static str, &'static str, bool)> {
    meta_for(name).map(|m| (m.name, m.group.as_str(), m.is_cross_file()))
}

/// Canonicalize every configured rule map and return deprecation warnings.
pub fn canonicalize_config(config: &mut FalconConfig) -> Vec<String> {
    let mut warnings = Vec::new();
    canonicalize_rules(&mut config.linter.rules, "linter.rules", &mut warnings);
    canonicalize_rules(
        &mut config.cross_file.rules,
        "cross-file.rules",
        &mut warnings,
    );
    for (index, ov) in config.overrides.iter_mut().enumerate() {
        if let Some(linter) = &mut ov.linter {
            canonicalize_rules(
                &mut linter.rules,
                &format!("overrides[{index}].linter.rules"),
                &mut warnings,
            );
        }
        if let Some(cross_file) = &mut ov.cross_file {
            canonicalize_rules(
                &mut cross_file.rules,
                &format!("overrides[{index}].cross-file.rules"),
                &mut warnings,
            );
        }
    }
    warnings
}

fn canonicalize_rules(rules: &mut Rules, path: &str, warnings: &mut Vec<String>) {
    for (group, entries) in &mut rules.groups {
        let canonical = canonicalize_entries(
            std::mem::take(entries),
            |value| rule_level_rank(value.level()),
            &format!("{path}.{group}"),
            warnings,
        );
        entries.extend(canonical);
        for (name, configuration) in entries {
            if let falcon_config::RuleConfiguration::WithOptions { options, .. } = configuration {
                warnings.extend(crate::canonicalize_option_keys(
                    options.value_mut(),
                    name,
                    &format!("{path}.{group}.{name}.options"),
                ));
            }
        }
    }
}

/// Rewrite one JSON rule map using the same collision policy as config loading.
///
/// The first occurrence determines the key's position. Highest severity wins;
/// canonical names win ties, then the first alias in [`RULE_ALIASES`] wins.
/// Unknown names and their values are preserved.
/// `path` is the enclosing section and group, used in every warning.
pub fn canonicalize_rule_map(
    entries: &mut serde_json::Map<String, serde_json::Value>,
    path: &str,
) -> Vec<String> {
    let (canonical, warnings) = canonicalize_rule_entries(std::mem::take(entries), path);
    *entries = canonical;
    warnings
}

/// Canonicalize JSON rule entries without losing duplicate input names.
///
/// Uses the same severity, canonical-name tie priority, and insertion ordering
/// as [`canonicalize_rule_map`].
/// `path` is the enclosing section and group, used in every warning.
pub fn canonicalize_rule_entries(
    entries: impl IntoIterator<Item = (String, serde_json::Value)>,
    path: &str,
) -> (serde_json::Map<String, serde_json::Value>, Vec<String>) {
    let mut warnings = Vec::new();
    let canonical = canonicalize_entries(
        entries,
        |value| {
            let level = value
                .as_str()
                .or_else(|| value.get("level").and_then(|v| v.as_str()));
            level
                .and_then(|level| falcon_config::level_from_str::<serde_json::Error>(level).ok())
                .map(rule_level_rank)
                .unwrap_or(0)
        },
        path,
        &mut warnings,
    );
    (canonical.into_iter().collect(), warnings)
}

fn canonicalize_entries<T: serde::Serialize>(
    entries: impl IntoIterator<Item = (String, T)>,
    rank: impl Fn(&T) -> u8,
    path: &str,
    warnings: &mut Vec<String>,
) -> Vec<(String, T)> {
    let entries = entries.into_iter();
    let capacity = entries.size_hint().0;
    let mut positions = HashMap::<String, usize>::with_capacity(capacity);
    let mut canonical_entries: Vec<(String, T)> = Vec::with_capacity(capacity);
    let mut priorities = Vec::with_capacity(capacity);
    for (name, value) in entries {
        let canonical = canonical_rule_name(&name);
        let alias_index = RULE_ALIASES.iter().position(|(alias, _)| *alias == name);
        if alias_index.is_some() {
            let key_path = if path.is_empty() {
                name.clone()
            } else {
                format!("{path}.{name}")
            };
            warnings.push(format!(
                "deprecated rule key `{key_path}`; use `{canonical}`; \
                 run `falcon migrate` to update"
            ));
        }
        let priority = (
            rank(&value),
            alias_index.is_none(),
            std::cmp::Reverse(alias_index.unwrap_or(0)),
        );
        if let Some(&index) = positions.get(canonical) {
            let discarded = if priority > priorities[index] {
                &canonical_entries[index].1
            } else {
                &value
            };
            let key_path = if path.is_empty() {
                canonical.to_owned()
            } else {
                format!("{path}.{canonical}")
            };
            let discarded =
                serde_json::to_string(discarded).expect("rule configurations serialize as JSON");
            warnings.push(format!(
                "duplicate rule key `{key_path}`: discarded configuration {discarded}; \
                 highest severity wins, then canonical names, then legacy alias priority"
            ));
            if priority > priorities[index] {
                canonical_entries[index].1 = value;
                priorities[index] = priority;
            }
        } else {
            let canonical = if canonical == name {
                name
            } else {
                canonical.to_owned()
            };
            positions.insert(canonical.clone(), canonical_entries.len());
            canonical_entries.push((canonical, value));
            priorities.push(priority);
        }
    }
    canonical_entries
}

/// Shared collision ordering: higher severity wins; `on` and `warn` tie.
pub fn rule_level_rank(level: RulePlainConfiguration) -> u8 {
    match level {
        RulePlainConfiguration::Off => 1,
        RulePlainConfiguration::Info => 2,
        RulePlainConfiguration::On | RulePlainConfiguration::Warn => 3,
        RulePlainConfiguration::Error => 4,
    }
}
