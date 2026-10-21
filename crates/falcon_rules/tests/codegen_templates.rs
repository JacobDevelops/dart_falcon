#[path = "scaffolds/cross_file_rule.rs"]
mod cross_file_rule;
#[path = "scaffolds/file_rule.rs"]
mod file_rule;

#[test]
fn scaffold_templates_implement_real_rule_traits() {
    fn file<T: falcon_analyze::Rule + Default>() {
        let _ = T::default();
    }
    fn project<T: falcon_analyze::CrossFileRule + Default>() {
        let _ = T::default();
    }

    file::<file_rule::ScaffoldRule>();
    project::<cross_file_rule::ScaffoldRule>();
}
