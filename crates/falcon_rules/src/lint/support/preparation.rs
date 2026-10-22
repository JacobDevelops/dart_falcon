macro_rules! prepare {
    ($kind:ident, $collector:ident, $ctx:ident, $diagnostics:ident) => {
        falcon_analyze::RulePreparation::$kind(Box::new($collector {
            $ctx,
            $diagnostics: Vec::new(),
        }))
    };
}

pub(crate) use prepare;
