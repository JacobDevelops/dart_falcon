use std::borrow::Cow;
use std::collections::HashMap;
use std::path::Path;

use falcon_config::FalconConfig;
use falcon_diagnostics::Diagnostic;

use crate::CheckOptions;
use crate::baseline::Baseline;

pub(crate) fn apply_baseline(
    options: &CheckOptions,
    config: &FalconConfig,
    root: &Path,
    diagnostics: &mut Vec<Diagnostic>,
    sources: &HashMap<Cow<'_, str>, &str>,
) -> Result<(), String> {
    let configured = config.baseline.as_ref().map(|path| root.join(path));
    let baseline_path = options.baseline.as_ref().or(configured.as_ref());
    if let Some(path) = &options.write_baseline {
        if diagnostics.iter().any(|diagnostic| {
            falcon_diagnostics::RESERVED_DIAGNOSTIC_IDS.contains(&diagnostic.rule)
        }) {
            return Err("Cannot write a baseline after syntax or operational errors.".into());
        }
        Baseline::from_diagnostics(diagnostics, root, |path| sources.get(path).copied())?
            .write(path)?;
        eprintln!("Baseline written to {}.", path.display());
    }
    let baseline = baseline_path.map(|path| Baseline::load(path)).transpose()?;
    if let Some(baseline) = baseline {
        let stats = baseline.apply(diagnostics, root, |path| sources.get(path).copied())?;
        eprintln!(
            "Baseline: {} suppressed, {} fixed (stale).",
            stats.suppressed, stats.stale
        );
    }
    Ok(())
}
