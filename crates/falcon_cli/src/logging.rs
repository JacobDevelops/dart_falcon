use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

use crate::args::LogFormat;

fn default_filter(verbose: bool) -> EnvFilter {
    let level = if verbose { "debug" } else { "info" };
    EnvFilter::new(format!("{level},falcon::stages=off"))
}

pub fn init(verbose: bool, log_format: LogFormat) {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| default_filter(verbose));

    match log_format {
        LogFormat::Json => {
            tracing_subscriber::registry()
                .with(filter)
                .with(
                    fmt::layer()
                        .json()
                        .with_span_events(fmt::format::FmtSpan::CLOSE)
                        .with_writer(std::io::stderr),
                )
                .init();
        }
        LogFormat::Text => {
            tracing_subscriber::registry()
                .with(filter)
                .with(
                    fmt::layer()
                        .with_target(false)
                        .with_span_events(fmt::format::FmtSpan::CLOSE)
                        .with_writer(std::io::stderr),
                )
                .init();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use tracing::{Subscriber, span::Attributes};
    use tracing_subscriber::{Layer, layer::Context, registry::LookupSpan};

    use super::*;

    type Record = (&'static str, Option<&'static str>, std::thread::ThreadId);
    struct Capture(Arc<Mutex<Vec<Record>>>);

    impl<S: Subscriber + for<'a> LookupSpan<'a>> Layer<S> for Capture {
        fn on_new_span(&self, attrs: &Attributes<'_>, id: &tracing::Id, ctx: Context<'_, S>) {
            let span = ctx.span(id).unwrap();
            self.0.lock().unwrap().push((
                attrs.metadata().name(),
                span.parent().map(|parent| parent.name()),
                std::thread::current().id(),
            ));
        }
    }

    #[test]
    fn cli_default_filters_keep_stages_off_including_verbose() {
        for verbose in [false, true] {
            let records = Arc::new(Mutex::new(Vec::new()));
            let subscriber = tracing_subscriber::registry()
                .with(default_filter(verbose))
                .with(Capture(records.clone()));
            tracing::subscriber::with_default(subscriber, || {
                let _stage = tracing::debug_span!(target: "falcon::stages", "stage").entered();
                let _detail = tracing::debug_span!(target: "falcon::detail", "detail").entered();
            });
            let names: Vec<_> = records
                .lock()
                .unwrap()
                .iter()
                .map(|record| record.0)
                .collect();
            assert_eq!(names, if verbose { vec!["detail"] } else { vec![] });
        }
    }
}
