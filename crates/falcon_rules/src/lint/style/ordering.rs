use falcon_analyze::AnalyzeContext;
use falcon_diagnostics::Diagnostic;
use falcon_syntax::cst::Identifier;

#[derive(Clone, Copy)]
pub(super) enum Order {
    Ascending,
    Descending,
}

impl Order {
    pub(super) fn configured(ctx: &AnalyzeContext<'_>, rule: &str) -> Option<Self> {
        match crate::lint::options::for_rule(ctx, rule)?
            .get("order")?
            .as_str()?
        {
            "ascending" => Some(Self::Ascending),
            "descending" => Some(Self::Descending),
            _ => None,
        }
    }

    pub(super) fn is_out_of_order(self, previous: &str, current: &str) -> bool {
        match self {
            Self::Ascending => previous > current,
            Self::Descending => previous < current,
        }
    }

    pub(super) fn check_names<'a>(
        self,
        ctx: &AnalyzeContext<'_>,
        rule: &'static str,
        names: impl Iterator<Item = Option<Identifier<'a>>>,
        diags: &mut Vec<Diagnostic>,
    ) {
        let mut previous = None;
        for name in names {
            if let Some(current) = name
                && previous.is_some_and(|previous| self.is_out_of_order(previous, current.name()))
            {
                diags.push(ctx.diagnostic(
                    rule,
                    "Reorder named entries to follow the configured order.",
                    current.span(),
                ));
            }
            previous = name.map(|name| name.name());
        }
    }
}
