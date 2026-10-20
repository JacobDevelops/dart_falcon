use crate::support::build_falcon;
use clap::Args;
use std::{path::PathBuf, process::Command};

#[derive(Args)]
pub(crate) struct Options {
    #[arg(
        long,
        help = "Corpus (default: apps/mobile/lib under required $JFIT_PATH)"
    )]
    corpus: Option<PathBuf>,
    #[arg(long, default_value_t = 5, help = "Timed runs to take the median of")]
    runs: usize,
    #[arg(
        long,
        default_value_t = 1000,
        help = "Wall-clock budget in milliseconds"
    )]
    budget_ms: u128,
    #[arg(long, help = "Falcon binary (default: build the release binary)")]
    falcon_bin: Option<PathBuf>,
    #[arg(
        long,
        requires = "falcon_bin",
        help = "Don't rebuild the release binary first"
    )]
    skip_build: bool,
    #[arg(long, help = "Output results as JSON")]
    json: bool,
}

fn perf_corpus(corpus: Option<PathBuf>, root: Option<std::ffi::OsString>) -> Option<PathBuf> {
    corpus.or_else(|| root.map(|root| PathBuf::from(root).join("apps/mobile/lib")))
}

pub(crate) fn run(options: Options) {
    let Options {
        corpus,
        runs,
        budget_ms,
        falcon_bin,
        skip_build: _,
        json: json_output,
    } = options;
    let runs = runs.max(1);

    let corpus = perf_corpus(corpus, std::env::var_os("JFIT_PATH")).unwrap_or_else(|| {
        eprintln!("error: set JFIT_PATH or pass --corpus");
        std::process::exit(1);
    });
    if !corpus.exists() {
        eprintln!("error: corpus path does not exist: {}", corpus.display());
        std::process::exit(1);
    }

    let falcon_bin = falcon_bin.unwrap_or_else(|| build_falcon(true));
    if !falcon_bin.exists() {
        eprintln!("error: falcon binary not found: {}", falcon_bin.display());
        std::process::exit(1);
    }

    // Warm-up run primes the OS file cache so timed runs measure the linter,
    // not cold disk I/O.
    let run_once = |label: &str| -> u128 {
        let start = std::time::Instant::now();
        let output = Command::new(&falcon_bin)
            // --exit-code 0 keeps "violations found" from reading as failure;
            // a non-zero exit therefore means the pipeline itself broke.
            .args(["check", "--quiet", "--exit-code", "0"])
            .arg(&corpus)
            .output()
            .expect("failed to spawn falcon");
        let elapsed = start.elapsed().as_millis();
        if !output.status.success() {
            eprintln!(
                "error: falcon check failed ({}): {}",
                label,
                String::from_utf8_lossy(&output.stderr)
            );
            std::process::exit(1);
        }
        elapsed
    };

    run_once("warm-up");
    let mut times: Vec<u128> = (0..runs)
        .map(|n| {
            let t = run_once(&format!("run {}", n + 1));
            eprintln!("  run {}: {}ms", n + 1, t);
            t
        })
        .collect();
    times.sort_unstable();
    let median = times[times.len() / 2];
    let min = times[0];
    let max = times[times.len() - 1];
    let pass = median < budget_ms;

    if json_output {
        let report = serde_json::json!({
            "corpus": corpus.display().to_string(),
            "runs": runs,
            "budget_ms": budget_ms,
            "min_ms": min,
            "median_ms": median,
            "max_ms": max,
            "pass": pass,
        });
        println!("{}", serde_json::to_string_pretty(&report).unwrap());
    } else {
        println!();
        println!("── perf-lock ───────────────────────────────────────────");
        println!("  Corpus:         {}", corpus.display());
        println!("  Runs:           {runs}");
        println!("  Budget:         {budget_ms}ms");
        println!("  Min:            {min}ms");
        println!("  Median:         {median}ms");
        println!("  Max:            {max}ms");
        println!("  Status:         {}", if pass { "PASS" } else { "FAIL" });
        println!("────────────────────────────────────────────────────────");
    }

    if !pass {
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn perf_lock_requires_root_unless_corpus_is_explicit() {
        use std::path::PathBuf;
        assert_eq!(super::perf_corpus(None, None), None);
        assert_eq!(
            super::perf_corpus(None, Some("/tmp/jfit".into())),
            Some(PathBuf::from("/tmp/jfit/apps/mobile/lib")),
        );
        assert_eq!(
            super::perf_corpus(Some(PathBuf::from("/tmp/corpus")), None),
            Some(PathBuf::from("/tmp/corpus")),
        );
    }
}
