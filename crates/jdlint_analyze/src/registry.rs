use crate::Rule;

/// Registry of enabled lint rules.
pub struct RuleRegistry {
    rules: Vec<Box<dyn Rule>>,
}

impl RuleRegistry {
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    pub fn register(&mut self, rule: Box<dyn Rule>) {
        self.rules.push(rule);
    }
}

impl Default for RuleRegistry {
    fn default() -> Self {
        Self::new()
    }
}
