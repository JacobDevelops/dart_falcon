use crate::workspace_root;
use falcon_rules::meta::RuleGroup;
use std::fs;

/// Convert `snake_case` to `PascalCase` for the rule struct name.
fn pascal_case(snake: &str) -> String {
    snake
        .split('_')
        .filter(|s| !s.is_empty())
        .map(|s| {
            let mut chars = s.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().chain(chars).collect::<String>(),
                None => String::new(),
            }
        })
        .collect()
}

/// Render the Rule impl stub for a new rule.
fn rule_stub(struct_name: &str) -> String {
    format!(
        r#"use falcon_analyze::{{AnalyzeContext, Rule, RulePreparation, StructuralRuleVisitor, StructuralTraversal}};
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::{{Expr, Program}};

#[derive(Default)]
pub struct {struct_name};

impl Rule for {struct_name} {{
    fn prepare<'a>(&self, program: Program<'a>, ctx: &'a AnalyzeContext<'a>) -> RulePreparation<'a> {{
        let _ = (program, ctx);
        RulePreparation::Structural(Box::new(Collector {{ diagnostics: Vec::new() }}))
    }}
}}

struct Collector {{
    diagnostics: Vec<Diagnostic>,
}}

falcon_analyze::structural_visitor! {{
impl<'a> StructuralRuleVisitor<'a> for Collector {{
    fn visit_expr(&mut self, node: Expr<'a>) -> StructuralTraversal {{
        let _ = node;
        StructuralTraversal::Continue
    }}

    fn finish(self: Box<Self>) -> Vec<Diagnostic> {{
        self.diagnostics
    }}
}}
}}
"#
    )
}

/// Generate a rule implementation stub plus corpus fixture skeletons.
///
/// `cargo xtask codegen rule --group suspicious --name avoid-foo`
///
/// The rule id is taken from `--name` verbatim (no kebab/snake derivation); the
/// module file is `lint/<group>/<name-with-dashes-as-underscores>.rs`.
pub(crate) fn run(group: String, rule_id: String) {
    // The module file name is the rule id with dashes normalized to underscores.
    let module = rule_id.replace('-', "_");

    let root = workspace_root();
    let rule_file = root.join(format!(
        "crates/falcon_rules/src/lint/{}/{}.rs",
        group, module
    ));
    let corpus_dir = root.join(format!("crates/falcon_rules/tests/corpus/{}", rule_id));

    if rule_file.exists() {
        eprintln!("error: {} already exists", rule_file.display());
        std::process::exit(1);
    }

    let struct_name = pascal_case(&module);
    fs::create_dir_all(rule_file.parent().expect("rule directory")).expect("create rule directory");
    fs::write(&rule_file, rule_stub(&struct_name)).expect("write rule stub");

    fs::create_dir_all(&corpus_dir).expect("create corpus dir");
    let bad = corpus_dir.join("bad.dart");
    let good = corpus_dir.join("good.dart");
    if !bad.exists() {
        fs::write(
            &bad,
            format!(
                "// TODO: violations annotated as /* expect: {} */\n",
                rule_id
            ),
        )
        .expect("write bad.dart");
    }
    if !good.exists() {
        fs::write(&good, "// TODO: compliant code, no annotations\n").expect("write good.dart");
    }

    println!("generated:");
    println!("  {}", rule_file.display());
    println!("  {}", bad.display());
    println!("  {}", good.display());
    println!();
    println!("next steps:");
    let group_module = root.join(format!("crates/falcon_rules/src/lint/{group}.rs"));
    if !group_module.exists() {
        println!(
            "  create crates/falcon_rules/src/lint/{group}.rs and add `pub mod {group};` to crates/falcon_rules/src/lint.rs"
        );
    }
    println!(
        "  1. add `pub mod {};` to crates/falcon_rules/src/lint/{}.rs",
        module, group
    );
    let category = RuleGroup::ALL
        .iter()
        .find(|category| category.as_str() == group)
        .unwrap();
    println!(
        "  2. add RuleMeta::file::<lint::{group}::{module}::{struct_name}>(\
         \"{rule_id}\", RuleGroup::{category:?}, FLUTTER /* or NONE */, recommended, source) to \
         RULE_METADATA (crates/falcon_rules/src/meta.rs); use resolved for semantic rules"
    );
    println!("  3. add `{rule_id}` under linter.rules.{group} in the root falcon.json showcase");
    println!(
        "  4. implement the rule, fill in fixtures, then run `cargo xtask validate-rules --rule {}`",
        rule_id
    );
}
