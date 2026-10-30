use falcon_analyze::AnalyzeContext;
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::Identifier;

#[derive(Clone, Copy, serde::Deserialize, serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Order {
    Ascending,
    Descending,
}

impl Order {
    pub(crate) fn is_out_of_order(self, previous: &str, current: &str) -> bool {
        match self {
            Self::Ascending => previous > current,
            Self::Descending => previous < current,
        }
    }

    pub(crate) fn check_names<'a>(
        self,
        ctx: &AnalyzeContext<'_>,
        names: impl Iterator<Item = Option<Identifier<'a>>>,
        diags: &mut Vec<Diagnostic>,
    ) {
        let mut previous = None;
        for name in names {
            if let Some(current) = name
                && previous.is_some_and(|previous| self.is_out_of_order(previous, current.name()))
            {
                diags.push(ctx.diagnostic(
                    "Reorder named entries to follow the configured order.",
                    current.span(),
                ));
            }
            previous = name.map(Identifier::name);
        }
    }
}
