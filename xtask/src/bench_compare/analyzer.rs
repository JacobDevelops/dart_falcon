//! Separate Dart analyzer evidence, not a Falcon self-regression threshold.
use super::full_analysis::{Case, selected_cases, source_revision, verify_clean_checkout};
use super::{
    MAX_RSS_KB, Options, Result, guarded_output, hyperfine_executable, machine_metadata,
    prepare_checkout, provenance, quote, resolved, statistics, text, write_json,
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};
use walkdir::WalkDir;

const ANALYZER_ARGS: [&str; 4] = ["--suppress-analytics", "analyze", "--format=json", "."];

fn findings(captured: &Output) -> Result<Value> {
    validate_findings(captured.status.code(), &captured.stdout, &captured.stderr)
}

fn validate_findings(exit_code: Option<i32>, stdout: &[u8], stderr: &[u8]) -> Result<Value> {
    if !matches!(exit_code, Some(0..=3)) {
        return Err(format!(
            "analyzer crashed or failed: {exit_code:?}: {}",
            String::from_utf8_lossy(stderr)
        )
        .into());
    }
    if !stderr.iter().all(u8::is_ascii_whitespace) {
        return Err(format!("analyzer stderr: {}", String::from_utf8_lossy(stderr)).into());
    }
    let report: Value = serde_json::from_slice(stdout)?;
    let diagnostics = report["diagnostics"]
        .as_array()
        .ok_or("missing analyzer diagnostics")?;
    if report["version"] != 1 {
        return Err("unsupported analyzer JSON version".into());
    }
    let mut counts = BTreeMap::<String, u64>::new();
    for diagnostic in diagnostics {
        let code = diagnostic["code"]
            .as_str()
            .filter(|code| !code.is_empty())
            .ok_or("missing analyzer diagnostic code")?;
        if !matches!(
            diagnostic["severity"].as_str(),
            Some("ERROR" | "WARNING" | "INFO")
        ) || !diagnostic["location"].is_object()
        {
            return Err("invalid analyzer diagnostic".into());
        }
        *counts.entry(code.to_owned()).or_default() += 1;
    }
    if exit_code != Some(0) && diagnostics.is_empty() {
        // Dart JSON omits priority pubspec/analysis_options errors.
        return Err("analyzer diagnostic exit without rendered diagnostics".into());
    }
    Ok(json!({"exitCode":exit_code,"diagnostics":diagnostics,
        "count":diagnostics.len(),"findingsByCode":counts}))
}

fn validate_resolved_work(work: &Value) -> Result<()> {
    let diagnostics = work["diagnostics"]
        .as_array()
        .ok_or("missing resolved diagnostics")?;
    if let Some(diagnostic) = diagnostics
        .iter()
        .find(|diagnostic| diagnostic["severity"] == "ERROR")
    {
        return Err(format!(
            "resolved analyzer workload has an error: {}",
            diagnostic["code"]
        )
        .into());
    }
    Ok(())
}

fn check_warm_count(baseline: u64, actual: u64) -> Result<()> {
    if baseline == 0 || actual != baseline {
        return Err(format!(
            "warm analyzer cache count drifted: settled {baseline}, observed {actual}"
        )
        .into());
    }
    Ok(())
}

fn populate_cache(minimum: u32, mut run: impl FnMut() -> Result<u64>) -> Result<Value> {
    let minimum = minimum.max(1);
    let maximum = minimum.checked_add(10).ok_or("warmup limit overflow")?;
    let mut counts = Vec::with_capacity(usize::try_from(maximum)?);
    let mut previous = None;
    for completed in 1..=maximum {
        let files = run()?;
        if files == 0 {
            return Err("warm population produced no persistent analyzer cache".into());
        }
        counts.push(files);
        if completed >= minimum && previous == Some(files) {
            return Ok(
                json!({"runs":completed,"minimumRuns":minimum,"maximumRuns":maximum,
                "persistentCacheFilesAfterRuns":counts,"settledFileCount":files}),
            );
        }
        previous = Some(files);
    }
    Err(format!("analyzer cache did not settle within {maximum} untimed warmup analyses").into())
}

fn cache_files(state: &Path) -> Result<u64> {
    let cache = state.join(".analysis-driver");
    if !cache.exists() {
        return Ok(0);
    }
    let mut count = 0;
    for entry in WalkDir::new(cache) {
        let entry = entry?;
        if entry.file_type().is_file() && entry.metadata()?.len() > 0 {
            count += 1;
        }
    }
    Ok(count)
}

fn command_text(dart: &Path) -> String {
    format!("{} {}", quote(dart), ANALYZER_ARGS.join(" "))
}

fn invocation(dart: &Path, project: &Path, state: &Path) -> Command {
    let mut command = Command::new(dart);
    command
        .current_dir(project)
        .args(ANALYZER_ARGS)
        .env("ANALYZER_STATE_LOCATION_OVERRIDE", state)
        .env_remove("DART_SDK")
        .env_remove("FLUTTER_ROOT");
    command
}

fn timed_sample(report: &Value) -> Result<(f64, i32)> {
    if report["schema_version"] != 2 {
        return Err("hyperfine 2 measurement schema required".into());
    }
    let results = report["results"]
        .as_array()
        .ok_or("missing Hyperfine results")?;
    if results.len() != 1 {
        return Err("expected one Hyperfine command".into());
    }
    let measurements = results[0]["measurements"]
        .as_array()
        .ok_or("missing Hyperfine measurements")?;
    if measurements.len() != 1 {
        return Err("expected one Hyperfine sample".into());
    }
    let sample = &measurements[0];
    let seconds = sample["time_wall_clock"]["value"]
        .as_f64()
        .ok_or("missing wall time")?;
    if sample["time_wall_clock"]["unit"] != "second" || !seconds.is_finite() || seconds <= 0.0 {
        return Err("invalid Hyperfine wall time".into());
    }
    let exit_code = i32::try_from(
        sample["exit_code"]
            .as_i64()
            .ok_or("missing analyzer exit code")?,
    )?;
    if !(0..=3).contains(&exit_code) {
        return Err(format!("timed analyzer crashed or failed: {exit_code}").into());
    }
    Ok((seconds, exit_code))
}

struct Measurement<'a> {
    dart: &'a Path,
    project: &'a Path,
    state: &'a Path,
    scratch: &'a Path,
    expected: &'a Value,
    hyperfine: &'a Path,
}

impl Measurement<'_> {
    fn sample(&self) -> Result<f64> {
        let stdout = self.scratch.join("analyzer.stdout");
        let stderr = self.scratch.join("analyzer.stderr");
        let export = self.scratch.join("analyzer-hyperfine.json");
        let invocation = invocation(self.dart, self.project, self.state);
        let command = format!(
            "exec {} > {} 2> {}",
            command_text(self.dart),
            quote(&stdout),
            quote(&stderr)
        );
        let mut timing = Command::new(self.hyperfine);
        timing
            .current_dir(self.project)
            .args([
                "--shell",
                "/bin/sh",
                "--warmup",
                "0",
                "--runs",
                "1",
                "--ignore-failure",
                "--export-json",
            ])
            .arg(&export)
            .arg(command);
        for (key, value) in invocation.get_envs() {
            if let Some(value) = value {
                timing.env(key, value);
            } else {
                timing.env_remove(key);
            }
        }
        let captured = guarded_output(
            &mut timing,
            &self.scratch.join("timing"),
            u64::from(MAX_RSS_KB),
        )?;
        if !captured.status.success() {
            return Err(format!("Hyperfine timing failed: {}", captured.status).into());
        }
        let measured: Value = serde_json::from_slice(&fs::read(export)?)?;
        let (elapsed, exit_code) = timed_sample(&measured)?;
        if validate_findings(Some(exit_code), &fs::read(stdout)?, &fs::read(stderr)?)?
            != *self.expected
        {
            return Err("analyzer findings changed between samples".into());
        }
        Ok(elapsed)
    }

    fn populate(&self, minimum: u32) -> Result<Value> {
        populate_cache(minimum, || {
            let captured = guarded_output(
                &mut invocation(self.dart, self.project, self.state),
                &self.scratch.join("population"),
                u64::from(MAX_RSS_KB),
            )?;
            if findings(&captured)? != *self.expected {
                return Err("analyzer findings changed during warm population".into());
            }
            cache_files(self.state)
        })
    }

    fn measure(&self, runs: u32, baseline: Option<u64>) -> Result<Value> {
        let capacity = usize::try_from(runs)?;
        let mut samples = Vec::with_capacity(capacity);
        let mut populated_files = Vec::with_capacity(capacity);
        for _ in 0..runs {
            if baseline.is_none() {
                if self.state.exists() {
                    fs::remove_dir_all(self.state)?;
                }
            } else if let Some(settled) = baseline {
                check_warm_count(settled, cache_files(self.state)?)?;
            }
            samples.push(self.sample()?);
            let files = cache_files(self.state)?;
            if files == 0 {
                return Err("analyzer did not persist .analysis-driver cache; cold/warm comparison is inapplicable".into());
            }
            if let Some(settled) = baseline {
                check_warm_count(settled, files)?;
            }
            populated_files.push(files);
        }
        Ok(json!({
            "seconds":statistics(&samples)?,
            "persistentCacheFilesBeforeSamples":baseline,
            "persistentCacheFilesAfterSamples":populated_files,
        }))
    }
}

pub(super) fn run(options: &Options) -> Result<()> {
    let root = crate::workspace_root();
    let destination = options
        .output
        .as_ref()
        .ok_or("analyzer comparison requires --output")?;
    let pins: Value = serde_json::from_str(include_str!("../../e2e-corpus.json"))?;
    let cases = selected_cases(options.analysis.resolved, &pins)?;
    let scratch = tempfile::tempdir()?;
    let hyperfine = hyperfine_executable(options)?;
    let portable_dart = if options.analysis.resolved {
        None
    } else {
        Some(fs::canonicalize(
            PathBuf::from(text(Command::new("mise").args(["where", "dart@3.13.6"]))?)
                .join("bin/dart"),
        )?)
    };
    let mut report = json!({
        "schemaVersion":1,"lane":"dart-analyzer-comparison", "resolved":options.analysis.resolved,
        "machine":machine_metadata()?,"runs":options.runs,"warmups":options.warmups,
        "repeatabilityEvidence":options.runs >= 10,
        "source":source_revision(&root)?,
        "statistics":"seconds; interpolated quartiles; unscaled MAD; sample standard-deviation / mean CV",
        "cache":{
            "applicability":"populated persistent byte store only; not a claim of steady-state cache",
            "cold":"fresh dart analyze and analysis-server processes; delete private ANALYZER_STATE_LOCATION_OVERRIDE before each sample",
            "warm":"fresh processes; retain populated .analysis-driver byte store after at least max(1,warmups) untimed analyses and two consecutive equal nonempty post-run file counts; fail if not settled within minimum+10 runs",
            "osPageCache":"retained in both modes; no claim of cold filesystem or retained in-memory server",
            "verification":"require nonempty cache after preflight and every sample; measured warm pre/post file counts must equal the settled baseline or the run fails",
            "stability":"file-count stationarity only, not unchanged bytes or in-memory server state; successful batches retain every requested sample; cold discards no samples",
            "source":"https://github.com/dart-lang/sdk/blob/3.13.6/pkg/analyzer/lib/file_system/physical_file_system.dart"
        },
        "timing":"Hyperfine 2 per-sample wall time includes fresh process startup, analysis, JSON rendering, shutdown and shell redirection; outer guard polling, capture reads, setup/cache deletion/output validation excluded",
        "comparability":"native Dart analysis and project analysis_options; not equivalent Falcon rule coverage or file-selection semantics",
        "diagnosticValidation":"exit 0..3 plus versioned JSON and stable complete rendered diagnostics; reject stderr, crashes, malformed output and nonzero exits with no rendered diagnostics; Dart JSON omits priority pubspec/analysis_options diagnostics",
        "resolvedDiagnosticPolicy":"known clean resolved pins reject every ERROR, including missing packages, unresolved imports, missing generated URIs and invalid part directives; INFO and WARNING remain valid findings",
        "limits":{"processTreeRssKiB":MAX_RSS_KB,"invocationSeconds":900},
        "results":[],"failures":[]
    });
    let mut failures = Vec::with_capacity(cases.len());
    let comparison = Comparison {
        options,
        root: &root,
        scratch: scratch.path(),
        hyperfine: &hyperfine,
        portable_dart: portable_dart.as_deref(),
    };
    for case in cases {
        let outcome = pins
            .as_array()
            .and_then(|pins| pins.iter().find(|pin| pin["id"] == case.corpus))
            .ok_or_else(|| "missing corpus pin".into())
            .and_then(|pin| comparison.measure_case(&case, pin));
        match outcome {
            Ok(result) => report["results"]
                .as_array_mut()
                .ok_or("invalid results")?
                .push(result),
            Err(error) => failures.push(format!("{}: {error}", case.id)),
        }
    }
    report["failures"] = json!(failures);
    write_json(destination, &report)?;
    if !failures.is_empty() {
        return Err(failures.join("; ").into());
    }
    Ok(())
}

struct Comparison<'a> {
    options: &'a Options,
    root: &'a Path,
    scratch: &'a Path,
    hyperfine: &'a Path,
    portable_dart: Option<&'a Path>,
}

impl Comparison<'_> {
    fn measure_case(&self, case: &Case, pin: &Value) -> Result<Value> {
        let Self {
            options,
            root,
            scratch,
            hyperfine,
            portable_dart,
        } = *self;
        let (project, dart) = if options.analysis.resolved {
            let (project, sdk) = resolved::validate(root, &case.id)?;
            (project, sdk.join("bin/dart"))
        } else {
            let checkout = root
                .join("target/bench-corpora/full-analysis")
                .join(&case.corpus);
            prepare_checkout(
                &checkout,
                pin["url"].as_str().ok_or("missing corpus URL")?,
                pin["revision"].as_str().ok_or("missing corpus revision")?,
            )?;
            verify_clean_checkout(&text(Command::new("git").current_dir(&checkout).args([
                "status",
                "--porcelain",
                "--ignored",
            ]))?)?;
            (
                checkout.join(&case.subpath),
                portable_dart.ok_or("missing portable SDK")?.to_path_buf(),
            )
        };
        if !project.is_dir() {
            return Err("missing analyzer case directory".into());
        }
        let dart = fs::canonicalize(dart)?;
        let version = text(Command::new(&dart).arg("--version"))?;
        if !options.analysis.resolved && !version.split_whitespace().any(|part| part == "3.13.6") {
            return Err(format!("portable analyzer requires Dart 3.13.6, found {version}").into());
        }
        let setup = if options.analysis.resolved {
            resolved::metadata(root, &case.id)?
        } else {
            Value::Null
        };
        // Private state never deletes the user's editor cache.
        let state = tempfile::tempdir_in(scratch)?;
        let preflight = guarded_output(
            &mut invocation(&dart, &project, state.path()),
            &scratch.join("preflight"),
            u64::from(MAX_RSS_KB),
        )?;
        let work = findings(&preflight)?;
        if options.analysis.resolved {
            validate_resolved_work(&work)?;
        }
        let preflight_cache_files = cache_files(state.path())?;
        if preflight_cache_files == 0 {
            return Err("analyzer did not persist .analysis-driver cache; cold/warm comparison is inapplicable".into());
        }
        let measurement = Measurement {
            dart: &dart,
            project: &project,
            state: state.path(),
            scratch,
            expected: &work,
            hyperfine,
        };
        let cold = measurement.measure(options.runs, None)?;
        let population = measurement.populate(options.warmups)?;
        let settled = population["settledFileCount"]
            .as_u64()
            .ok_or("missing settled cache count")?;
        let mut warm = measurement.measure(options.runs, Some(settled))?;
        warm["population"] = population;
        if options.analysis.resolved {
            resolved::validate(root, &case.id)?;
        }
        Ok(json!({"id":case.id,"corpus":pin,"subpath":case.subpath,
            "manifestFiles":case.files,"dartVersion":version,"dart":provenance(&dart)?,"setup":setup,
            "command":format!("ANALYZER_STATE_LOCATION_OVERRIDE=<private-state> {}",command_text(&dart)),
            "workingDirectory":project,"cacheApplicable":true,"preflightCacheFiles":preflight_cache_files,
            "work":work,"cold":cold,"warm":warm}))
    }
}

#[cfg(test)]
mod tests {
    use super::super::full_analysis::cases;
    use super::{
        ANALYZER_ARGS, cache_files, check_warm_count, findings, invocation, populate_cache,
        timed_sample, validate_findings, validate_resolved_work,
    };
    use serde_json::{Value, json};
    use std::os::unix::process::ExitStatusExt as _;
    use std::{fs, path::Path, process::Output};

    #[test]
    fn case_validation_rejects_escape_duplicate_and_missing_pin() {
        let pins = json!([{"id":"p"}]);
        for source in [
            "[]",
            r#"[{"id":"x","corpus":"p","subpath":"../escape","files":1}]"#,
            r#"[{"id":"x","corpus":"missing","subpath":".","files":1}]"#,
            r#"[{"id":"x","corpus":"p","subpath":"/absolute","files":1}]"#,
            r#"[{"id":"x","corpus":"p","subpath":".","files":0}]"#,
            r#"[{"id":"x","corpus":"p","subpath":".","files":1},{"id":"x","corpus":"p","subpath":".","files":1}]"#,
        ] {
            assert!(cases(source, &pins).is_err());
        }
        assert!(
            cases(
                r#"[{"id":"x","corpus":"p","subpath":"lib","files":1}]"#,
                &pins
            )
            .is_ok()
        );
    }

    fn captured(code: i32, stdout: &Value) -> Output {
        Output {
            status: std::process::ExitStatus::from_raw(code << 8),
            stdout: serde_json::to_vec(stdout).unwrap(),
            stderr: Vec::new(),
        }
    }

    #[test]
    fn findings_require_completed_structured_analysis() {
        let diagnostic = json!({"code":"undefined_identifier","severity":"ERROR","location":{}});
        let output = json!({"version":1,"diagnostics":[diagnostic,diagnostic]});
        let work = findings(&captured(3, &output)).unwrap();
        assert_eq!(work["findingsByCode"]["undefined_identifier"], json!(2));
        assert!(findings(&captured(4, &output)).is_err());
        assert!(findings(&captured(3, &json!({"version":1,"diagnostics":[]}))).is_err());
        assert!(findings(&captured(0, &json!({"diagnostics":[]}))).is_err());
        assert!(findings(&captured(0, &json!({"version":1,"diagnostics":[{}]}))).is_err());
        assert!(findings(&captured(0, &json!({"version":1,"diagnostics":[]}))).is_ok());
        let mut failed = captured(0, &output);
        failed.stderr = b"analysis server error".to_vec();
        assert!(findings(&failed).is_err());
        let mut signaled = captured(0, &json!({"version":1,"diagnostics":[]}));
        signaled.status = std::process::ExitStatus::from_raw(9);
        assert!(findings(&signaled).is_err());
        let mut malformed = captured(0, &json!({}));
        malformed.stdout = b"usage: dart analyze".to_vec();
        assert!(findings(&malformed).is_err());
    }

    #[test]
    fn cache_requires_nonempty_driver_files() {
        let state = tempfile::tempdir().unwrap();
        assert_eq!(cache_files(state.path()).unwrap(), 0);
        let cache = state.path().join(".analysis-driver");
        fs::create_dir_all(&cache).unwrap();
        fs::write(cache.join("empty"), []).unwrap();
        assert_eq!(cache_files(state.path()).unwrap(), 0);
        fs::write(cache.join("summary"), b"cached bytes").unwrap();
        assert_eq!(cache_files(state.path()).unwrap(), 1);
    }

    #[test]
    fn invocation_isolates_cache_and_disables_analytics() {
        let command = invocation(
            Path::new("/sdk/bin/dart"),
            Path::new("/project"),
            Path::new("/private-state"),
        );
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            ANALYZER_ARGS.map(std::ffi::OsStr::new)
        );
        assert!(
            command
                .get_envs()
                .any(|(key, value)| key == "ANALYZER_STATE_LOCATION_OVERRIDE"
                    && value == Some(std::ffi::OsStr::new("/private-state")))
        );
        assert_eq!(command.get_current_dir(), Some(Path::new("/project")));
    }
    #[test]
    fn accepts_precise_times_and_diagnostic_exit_codes() {
        let diagnostic = json!({"version":1,"diagnostics":[{"code":"sample",
            "severity":"ERROR","location":{}}]});
        let stdout = serde_json::to_vec(&diagnostic).unwrap();
        for exit_code in 0..=3 {
            let report = json!({"schema_version":2,"results":[{"measurements":[{
                "time_wall_clock":{"value":0.012_345_678,"unit":"second"},"exit_code":exit_code
            }]}]});
            assert_eq!(timed_sample(&report).unwrap(), (0.012_345_678, exit_code));
            let first = validate_findings(Some(exit_code), &stdout, b"").unwrap();
            assert_eq!(
                first,
                validate_findings(Some(exit_code), &stdout, b"").unwrap()
            );
            assert_eq!(first["exitCode"], exit_code);
        }
    }

    #[test]
    fn rejects_missing_failed_or_ambiguous_timing_samples() {
        let valid = json!({"schema_version":2,"results":[{"measurements":[{
            "time_wall_clock":{"value":1.0,"unit":"second"},"exit_code":0
        }]}]});
        for exit_code in [json!(4), json!(-1), Value::Null, json!("0")] {
            let mut report = valid.clone();
            report["results"][0]["measurements"][0]["exit_code"] = exit_code;
            assert!(timed_sample(&report).is_err());
        }
        for seconds in [json!(0), json!(-1), Value::Null] {
            let mut report = valid.clone();
            report["results"][0]["measurements"][0]["time_wall_clock"]["value"] = seconds;
            assert!(timed_sample(&report).is_err());
        }
        let mut wrong_unit = valid.clone();
        wrong_unit["results"][0]["measurements"][0]["time_wall_clock"]["unit"] =
            json!("millisecond");
        assert!(timed_sample(&wrong_unit).is_err());
        for report in [
            json!({"schema_version":1,"results":[]}),
            json!({"schema_version":2,"results":[]}),
            json!({"schema_version":2,"results":[{"measurements":[]}]}),
            json!({"schema_version":2,"results":[{"measurements":[{},{}]}]}),
            json!({"schema_version":2,"results":[{},{}]}),
        ] {
            assert!(timed_sample(&report).is_err());
        }
    }

    #[test]
    fn resolved_work_accepts_information_but_rejects_resolution_errors() {
        assert!(validate_resolved_work(&json!({"diagnostics":[]})).is_ok());
        assert!(
            validate_resolved_work(&json!({"diagnostics":[
                {"severity":"INFO","code":"prefer_const_constructors"},
                {"severity":"WARNING","code":"unused_import"}
            ]}))
            .is_ok()
        );
        for code in [
            "uri_does_not_exist",
            "uri_has_not_been_generated",
            "part_of_non_part",
            "undefined_identifier",
        ] {
            let work = json!({"diagnostics":[{"severity":"ERROR","code":code}]});
            let error = validate_resolved_work(&work).unwrap_err().to_string();
            assert!(error.contains(code), "{error}");
        }
        assert!(validate_resolved_work(&json!({})).is_err());
    }

    #[test]
    fn warm_population_requires_consecutive_counts_and_honors_minimum() {
        let mut values = [10, 11, 11].into_iter();
        let population = populate_cache(1, || Ok(values.next().unwrap())).unwrap();
        assert_eq!(population["runs"], 3);
        assert_eq!(population["settledFileCount"], 11);
        assert_eq!(
            population["persistentCacheFilesAfterRuns"],
            json!([10, 11, 11])
        );
        let mut runs = 0;
        let population = populate_cache(4, || {
            runs += 1;
            Ok(10)
        })
        .unwrap();
        assert_eq!(runs, 4);
        assert_eq!(population["runs"], 4);
    }

    #[test]
    fn warm_population_fails_at_cap_or_empty_cache() {
        let mut runs = 0;
        assert!(
            populate_cache(1, || {
                runs += 1;
                Ok(runs)
            })
            .is_err()
        );
        assert_eq!(runs, 11);
        assert!(populate_cache(1, || Ok(0)).is_err());
        assert!(populate_cache(u32::MAX, || Ok(10)).is_err());
    }

    #[test]
    fn measured_warm_count_drift_fails_closed() {
        assert!(check_warm_count(10, 10).is_ok());
        for actual in [0, 9, 11] {
            assert!(check_warm_count(10, actual).is_err());
        }
        assert!(check_warm_count(0, 0).is_err());
    }
}
