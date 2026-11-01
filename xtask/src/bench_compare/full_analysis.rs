use super::*;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Case {
    pub(super) id: String,
    pub(super) corpus: String,
    pub(super) subpath: String,
    pub(super) files: u64,
}

pub(super) fn cases(source: &str, pins: &Value) -> Result<Vec<Case>> {
    let cases: Vec<Case> = serde_json::from_str(source)?;
    validate_cases(&cases, pins)?;
    Ok(cases)
}

fn validate_cases(cases: &[Case], pins: &Value) -> Result<()> {
    let mut ids = BTreeSet::new();
    if cases.is_empty() {
        return Err("empty full-analysis case table".into());
    }
    for case in cases {
        if case.files == 0
            || !ids.insert(&case.id)
            || case.id.is_empty()
            || !Path::new(&case.subpath).components().all(|part| {
                matches!(
                    part,
                    std::path::Component::Normal(_) | std::path::Component::CurDir
                )
            })
            || !pins
                .as_array()
                .ok_or("invalid corpus manifest")?
                .iter()
                .any(|pin| pin["id"] == case.corpus)
        {
            return Err(format!("invalid full-analysis case: {case:?}").into());
        }
    }
    Ok(())
}

pub(super) fn selected_cases(resolved: bool, pins: &Value) -> Result<Vec<Case>> {
    let mut portable = cases(include_str!("../../full-analysis-cases.json"), pins)?;
    let portable_count = portable.len();
    portable.extend(super::resolved::cases());
    validate_cases(&portable, pins)?;
    let resolved_cases = portable.split_off(portable_count);
    Ok(if resolved { resolved_cases } else { portable })
}

fn verify(
    captured: &Output,
    files: u64,
    rules: u64,
    cross_file: u64,
    resolved: bool,
) -> Result<Value> {
    let stderr = std::str::from_utf8(&captured.stderr)?;
    if !captured.status.success() {
        return Err(format!("process exit check failed: {}", captured.status).into());
    }
    let diagnostics: Vec<Value> = serde_json::from_slice(&captured.stdout)?;
    let mut findings = std::collections::BTreeMap::<String, u64>::new();
    for diagnostic in &diagnostics {
        if matches!(
            diagnostic["rule"].as_str(),
            Some(
                "syntax-error"
                    | "internal-error"
                    | "rules-thread-error"
                    | "config"
                    | "unmatched-path"
                    | "incomplete-analysis"
            )
        ) {
            return Err(format!("full-analysis failure diagnostic: {diagnostic}").into());
        }
        let rule = diagnostic["rule"]
            .as_str()
            .ok_or("finding missing rule id")?;
        *findings.entry(rule.to_owned()).or_default() += 1;
    }
    let mut summary = None;
    let mut external = None;
    let mut completed = 0;
    let mut preparations = 0;
    let mut cross_file_completed = 0;
    let mut semantic_indexes = 0;
    for line in stderr.lines() {
        let record: Value = serde_json::from_str(line)?;
        if matches!(record["level"].as_str(), Some("WARN" | "ERROR")) {
            return Err(format!("full-analysis warning/failure: {line}").into());
        }
        let fields = &record["fields"];
        match fields["message"].as_str() {
            Some("starting check") => {
                if summary.replace(fields.clone()).is_some() {
                    return Err("multiple analysis summaries".into());
                }
            }
            Some("external libraries loaded") => {
                if fields["parse_error_files"].as_u64() != Some(0) {
                    return Err("missing or nonzero external parse_error_files".into());
                }
                external = fields["libraries"].as_u64();
            }
            Some("file analysis complete") => completed += 1,
            Some("close") if record["span"]["name"] == "registry_preparation" => preparations += 1,
            Some("close") if record["span"]["name"] == "cross_file_rule" => {
                cross_file_completed += 1;
            }
            Some("close") if record["span"]["name"] == "semantic_index_build" => {
                semantic_indexes += 1;
            }
            _ => {}
        }
    }
    let summary = summary.ok_or("missing analysis summary")?;
    if summary["file_count"] != files
        || summary["rule_count"] != rules
        || summary["cross_file_rule_count"] != cross_file
        || summary["resolve"] != true
        || completed != files
        || preparations != files
        || external.is_none_or(|libraries| (libraries == 0) == resolved)
        || cross_file_completed != cross_file
        || semantic_indexes == 0
    {
        return Err(format!("work mismatch: expected {files} files, {rules} file rules, {cross_file} cross-file rules, resolution; summary={summary}, completed={completed}, preparations={preparations}, external={external:?}, cross_file_completed={cross_file_completed}, semantic_indexes={semantic_indexes}").into());
    }
    Ok(
        json!({"files":files,"fileRules":rules,"crossFileRules":cross_file,"completedCrossFileRules":cross_file_completed,"semanticIndexBuilds":semantic_indexes,"resolution":true,"completedFiles":completed,"registryPreparations":preparations,"externalLibraries":external,"diagnostics":diagnostics.len(),"findingsByRule":findings}),
    )
}

#[derive(Debug)]
struct WorkMismatch(Value);

impl std::fmt::Display for WorkMismatch {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "expected-work check failed; review before --bless: {}",
            self.0
        )
    }
}

impl std::error::Error for WorkMismatch {}

fn work_differences(actual: &Value, expected: &Value) -> Value {
    let keys = actual
        .as_object()
        .into_iter()
        .chain(expected.as_object())
        .flat_map(|object| object.keys())
        .collect::<BTreeSet<_>>();
    let mut differences = serde_json::Map::new();
    for key in keys {
        if actual[key] == expected[key] {
            continue;
        }
        let difference = if key == "findingsByRule" {
            work_differences(&actual[key], &expected[key])
        } else {
            let delta = actual[key]
                .as_u64()
                .or_else(|| actual[key].is_null().then_some(0))
                .zip(
                    expected[key]
                        .as_u64()
                        .or_else(|| expected[key].is_null().then_some(0)),
                )
                .map(|(actual, expected)| i128::from(actual) - i128::from(expected));
            json!({"actual":actual[key],"expected":expected[key],"delta":delta})
        };
        differences.insert(key.clone(), difference);
    }
    Value::Object(differences)
}

fn verify_expected(actual: &Value, expected: &Value) -> Result<()> {
    if actual != expected {
        return Err(Box::new(WorkMismatch(work_differences(actual, expected))));
    }
    Ok(())
}

fn expectations(source: &[u8]) -> Result<Value> {
    let expected: Value = serde_json::from_slice(source)?;
    let object = expected
        .as_object()
        .ok_or("expected-work file must be an object")?;
    if object.values().any(|value| !value.is_object()) {
        return Err("expected-work entries must be objects".into());
    }
    Ok(expected)
}

pub(super) fn source_revision(root: &Path) -> Result<Value> {
    let revision = Command::new("jj")
        .current_dir(root)
        .args([
            "log",
            "-r",
            "@",
            "--no-graph",
            "-T",
            "commit_id ++ \"\\n\" ++ if(empty, \"true\", \"false\")",
        ])
        .output();
    if let Ok(output) = revision
        && output.status.success()
    {
        let output = String::from_utf8(output.stdout)?;
        let (revision, empty) = output
            .trim()
            .split_once('\n')
            .ok_or("invalid jj revision output")?;
        return Ok(
            json!({"vcs":"jj","revision":revision,"workingChangeEmpty":empty.parse::<bool>()?}),
        );
    }
    Ok(json!({
        "vcs":"git",
        "revision":text(Command::new("git").current_dir(root).args(["rev-parse","HEAD"]))?,
        "workingChangeEmpty":text(Command::new("git").current_dir(root).args(["status","--porcelain"]))?.is_empty()
    }))
}

pub(super) fn verify_clean_checkout(status: &str) -> Result<()> {
    if !status.trim().is_empty() {
        return Err(
            "clean corpus check failed: modified, untracked or ignored files present".into(),
        );
    }
    Ok(())
}

pub(super) fn run(options: &Options) -> Result<()> {
    let root = crate::workspace_root();
    let pins: Value = serde_json::from_str(include_str!("../../e2e-corpus.json"))?;
    let cases = selected_cases(options.analysis.resolved, &pins)?;
    let destination = options
        .output
        .as_ref()
        .ok_or("full-analysis requires --output")?;
    let binary = fs::canonicalize(falcon_executable(options)?)?;
    let scratch = tempfile::tempdir()?;
    fs::copy(&binary, scratch.path().join("falcon"))?;
    let binary = scratch.path().join("falcon");
    let cache = root.join("target/bench-corpora/full-analysis");
    let mut config = json!({"linter":{"rules":{"recommended":false}},"cross-file":{"rules":{"recommended":false}}});
    let mut rules = 0_u64;
    let mut cross_file = 0_u64;
    for rule in RULE_METADATA {
        let section = if matches!(rule.kind, RuleKind::CrossFile { .. }) {
            cross_file += 1;
            "cross-file"
        } else {
            rules += 1;
            "linter"
        };
        config[section]["rules"][rule.group.as_str()][rule.name] = json!("warn");
    }
    let hyperfine = hyperfine_executable(options)?;
    let expected_path = root.join("xtask/full-analysis-expected.json");
    let mut expected = expectations(&fs::read(&expected_path)?)?;
    if options.bless {
        let ids = selected_cases(false, &pins)?
            .into_iter()
            .chain(selected_cases(true, &pins)?)
            .map(|case| case.id)
            .collect::<BTreeSet<_>>();
        expected
            .as_object_mut()
            .ok_or("invalid expected-work object")?
            .retain(|id, _| ids.contains(id));
    }
    let mut report = json!({
        "schemaVersion":1, "lane":"uncached-full-analysis",
        "rustc":text(Command::new("rustc").arg("-vV"))?,
        "cargo":text(Command::new("cargo").arg("--version"))?,
        "machine":machine_metadata()?,
        "hyperfine":text(Command::new(&hyperfine).arg("--version"))?,
        "falconVersion":text(Command::new(&binary).arg("--version"))?,
        "binarySha256":text(Command::new("sha256sum").arg(&binary))?.split_whitespace().next(),
        "falconSourceRevision":source_revision(&root)?,
        "rendering":"full JSON findings rendered in timed runs; no rendering cap; rendering cost included",
        "configuration":config, "threads":4,
        "sdk":if options.analysis.resolved { "pinned SDK and package configuration; validated before and after timing" } else { "absent: PATH, DART_SDK and FLUTTER_ROOT removed; no pub get" },
        "cache":"fresh process, uncached analysis; OS page cache retained; no persistent analyzer state",
        "runs":options.runs, "warmups":options.warmups,
        "limits":{"processTreeRssKiB":MAX_RSS_KB,"batchSeconds":900,"rejectProcessSwap":true},
        "failures":[], "workDifferences":{}, "results":[]
    });
    let lane = Lane {
        options,
        binary,
        scratch: scratch.path(),
        cache,
        config,
        hyperfine,
        rules,
        cross_file,
    };
    let mut failures = Vec::new();
    for case in cases {
        let pin = pins
            .as_array()
            .and_then(|pins| pins.iter().find(|pin| pin["id"] == case.corpus));
        let outcome = pin
            .ok_or_else(|| "missing pin".into())
            .and_then(|pin| lane.measure_case(&case, pin, &mut expected));
        match outcome {
            Ok(result) => report["results"]
                .as_array_mut()
                .ok_or("invalid results")?
                .push(result),
            Err(error) => {
                if let Some(mismatch) = error.downcast_ref::<WorkMismatch>() {
                    report["workDifferences"][&case.id] = mismatch.0.clone();
                }
                failures.push(format!("{}: {error}", case.id));
            }
        }
    }
    report["failures"] = json!(failures);
    write_json(destination, &report)?;
    if !failures.is_empty() {
        return Err(failures.join("; ").into());
    }
    if options.bless {
        write_json(&expected_path, &expected)?;
    }
    Ok(())
}

struct Lane<'a> {
    options: &'a Options,
    binary: PathBuf,
    scratch: &'a Path,
    cache: PathBuf,
    config: Value,
    hyperfine: PathBuf,
    rules: u64,
    cross_file: u64,
}

impl Lane<'_> {
    fn checkout(&self, case: &Case, pin: &Value) -> Result<(PathBuf, Option<PathBuf>)> {
        if self.options.analysis.resolved {
            let (target, sdk) = resolved::validate(&crate::workspace_root(), &case.id)?;
            let checkout = self
                .cache
                .parent()
                .ok_or("missing cache parent")?
                .join("resolved")
                .join(&case.corpus);
            if target != checkout.join(&case.subpath) {
                return Err("resolved target differs from case manifest".into());
            }
            Ok((checkout, Some(sdk)))
        } else {
            let checkout = self.cache.join(&case.corpus);
            prepare_checkout(
                &checkout,
                pin["url"].as_str().ok_or("missing URL")?,
                pin["revision"].as_str().ok_or("missing revision")?,
            )?;
            let status = text(Command::new("git").current_dir(&checkout).args([
                "status",
                "--porcelain",
                "--ignored",
            ]))?;
            verify_clean_checkout(&status)?;
            Ok((checkout, None))
        }
    }

    fn measure_case(&self, case: &Case, pin: &Value, expected: &mut Value) -> Result<Value> {
        let (checkout, sdk) = self.checkout(case, pin)?;
        let mut config = self.config.clone();
        let mut includes = vec![json!("**")];
        if self.options.analysis.resolved {
            includes.extend(EXCLUDED.iter().map(|pattern| json!(format!("!{pattern}"))));
        }
        if let Some(excludes) = pin["excludes"].as_array() {
            for exclude in excludes {
                includes.push(json!(format!(
                    "!**/{}",
                    exclude.as_str().ok_or("invalid pin exclusion")?
                )));
            }
        }
        config["files"]["includes"] = json!(includes);
        let config_path = self.scratch.join("falcon.json");
        write_json(&config_path, &config)?;
        let target = checkout.join(&case.subpath);
        if !target.is_dir() {
            return Err("missing case target".into());
        }
        let command = |instrumented| {
            let mut command = Command::new(&self.binary);
            command.current_dir(&checkout).arg("check").arg(&target).arg("--config").arg(&config_path)
                .args(["--format","json","--exit-code","0","--log-format","json"])
                .env_remove("PATH").env_remove("DART_SDK").env_remove("FLUTTER_ROOT")
                .env("RAYON_NUM_THREADS","4")
                .env("RUST_LOG", if instrumented { "warn,falcon::stages=debug,falcon::detail=debug,falcon_project=info,falcon_analyze::parallel=info" } else { "warn" });
            if let Some(sdk) = &sdk {
                command.env("DART_SDK", sdk);
            }
            command
        };
        let preflight_command = command(true);
        let rss_path = self.scratch.join("rss");
        let mut capture = rss_command(&rss_path, &self.binary);
        capture
            .args(preflight_command.get_args())
            .current_dir(&checkout);
        for (key, value) in preflight_command.get_envs() {
            if let Some(value) = value {
                capture.env(key, value);
            } else {
                capture.env_remove(key);
            }
        }
        let preflight = guarded_output(
            &mut capture,
            &self.scratch.join("preflight"),
            u64::from(MAX_RSS_KB),
        )?;
        let peak_rss_mb = peak_rss_mb(&rss_path)?;
        let work = verify(
            &preflight,
            case.files,
            self.rules,
            self.cross_file,
            self.options.analysis.resolved,
        )?;
        if self.options.bless {
            expected[&case.id] = work.clone();
        } else {
            verify_expected(&work, &expected[&case.id])?;
        }
        let seconds = self.timing(&command(false))?;
        let postflight = guarded_output(
            &mut command(true),
            &self.scratch.join("postflight"),
            u64::from(MAX_RSS_KB),
        )?;
        let after = verify(
            &postflight,
            case.files,
            self.rules,
            self.cross_file,
            self.options.analysis.resolved,
        )?;
        verify_expected(&after, &expected[&case.id])?;
        if postflight.stdout != preflight.stdout {
            return Err("findings changed during timing".into());
        }
        let setup = if self.options.analysis.resolved {
            resolved::validate(&crate::workspace_root(), &case.id)?;
            resolved::metadata(&crate::workspace_root(), &case.id)?
        } else {
            Value::Null
        };
        Ok(
            json!({"id":case.id,"corpus":pin,"subpath":case.subpath,"configuration":config,"work":work,"setup":setup,"peakRssMb":peak_rss_mb,"seconds":seconds}),
        )
    }

    fn timing(&self, invocation: &Command) -> Result<Value> {
        let export = self.scratch.join("hyperfine.json");
        let arguments = invocation
            .get_args()
            .map(|arg| quote(Path::new(arg)))
            .collect::<Vec<_>>();
        let invocation_text = format!("{} {}", quote(&self.binary), arguments.join(" "));
        let mut timing = Command::new(&self.hyperfine);
        timing
            .current_dir(
                invocation
                    .get_current_dir()
                    .ok_or("missing timing directory")?,
            )
            .args([
                "--shell",
                "none",
                "--warmup",
                &self.options.warmups.to_string(),
                "--runs",
                &self.options.runs.to_string(),
                "--export-json",
            ])
            .arg(&export)
            .arg(&invocation_text);
        for (key, value) in invocation.get_envs() {
            if let Some(value) = value {
                timing.env(key, value);
            } else {
                timing.env_remove(key);
            }
        }
        let timed = guarded_output(
            &mut timing,
            &self.scratch.join("timing"),
            u64::from(MAX_RSS_KB),
        )?;
        if !timed.status.success() {
            return Err(format!("timing process failed: {}", timed.status).into());
        }
        let measured: Value = serde_json::from_slice(&fs::read(export)?)?;
        let measurements = measured["results"][0]["measurements"]
            .as_array()
            .ok_or("missing measurements (hyperfine 2 required)")?;
        if measurements.len() != self.options.runs as usize
            || measurements.iter().any(|sample| sample["exit_code"] != 0)
        {
            return Err("missing or failed timed samples".into());
        }
        let samples = measurements
            .iter()
            .map(|sample| {
                sample["time_wall_clock"]["value"]
                    .as_f64()
                    .ok_or("invalid wall time")
            })
            .collect::<std::result::Result<Vec<_>, _>>()?;
        statistics(&samples)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_case_table_and_pinned_ids() {
        let pins = json!([{"id":"bloc"}]);
        assert!(
            cases(
                r#"[{"id":"dart","corpus":"bloc","subpath":"packages/bloc","files":43}]"#,
                &pins
            )
            .is_ok()
        );
        for source in [
            "[]",
            r#"[{"id":"x","corpus":"missing","subpath":".","files":1}]"#,
            r#"[{"id":"x","corpus":"bloc","subpath":"../x","files":1}]"#,
            r#"[{"id":"x","corpus":"bloc","subpath":".","files":0}]"#,
        ] {
            assert!(cases(source, &pins).is_err());
        }
        let mut combined = cases(
            r#"[{"id":"x","corpus":"bloc","subpath":".","files":1}]"#,
            &pins,
        )
        .unwrap();
        combined.extend(
            cases(
                r#"[{"id":"x","corpus":"bloc","subpath":"packages/bloc","files":43}]"#,
                &pins,
            )
            .unwrap(),
        );
        assert!(validate_cases(&combined, &pins).is_err());
    }
    #[test]
    fn real_cases_resolve_against_pinned_public_corpus() {
        let pins = serde_json::from_str(include_str!("../../e2e-corpus.json")).unwrap();
        assert_eq!(selected_cases(false, &pins).unwrap().len(), 3);
        assert_eq!(selected_cases(true, &pins).unwrap().len(), 2);
    }
    fn captured(stderr: &str) -> Output {
        Output {
            status: Command::new("true").status().unwrap(),
            stdout: b"[]".to_vec(),
            stderr: stderr.as_bytes().to_vec(),
        }
    }
    fn records() -> Vec<Value> {
        vec![
            json!({"fields":{"message":"starting check","file_count":1,"rule_count":2,"cross_file_rule_count":1,"resolve":true}}),
            json!({"fields":{"message":"external libraries loaded","libraries":0,"parse_error_files":0}}),
            json!({"fields":{"message":"file analysis complete"}}),
            json!({"fields":{"message":"close"},"span":{"name":"registry_preparation"}}),
            json!({"fields":{"message":"close"},"span":{"name":"cross_file_rule"}}),
            json!({"fields":{"message":"close"},"span":{"name":"semantic_index_build"}}),
        ]
    }
    #[test]
    fn verifies_complete_work_and_rejects_each_missing_stage() {
        let records = records();
        let lines = records.iter().map(Value::to_string).collect::<Vec<_>>();
        assert!(verify(&captured(&lines.join("\n")), 1, 2, 1, false).is_ok());
        for (omitted, _) in lines.iter().enumerate() {
            let partial = lines
                .iter()
                .enumerate()
                .filter(|(index, _)| *index != omitted)
                .map(|(_, line)| line.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            assert!(
                verify(&captured(&partial), 1, 2, 1, false).is_err(),
                "omitted {omitted}"
            );
        }
        for (files, rules, cross) in [(2, 2, 1), (1, 3, 1), (1, 2, 2)] {
            assert!(verify(&captured(&lines.join("\n")), files, rules, cross, false).is_err());
        }
    }
    #[test]
    fn resolved_work_requires_external_libraries_and_exact_expectations() {
        let mut records = records();
        let output = |records: &[Value]| {
            captured(
                &records
                    .iter()
                    .map(Value::to_string)
                    .collect::<Vec<_>>()
                    .join("\n"),
            )
        };
        assert!(verify(&output(&records), 1, 2, 1, true).is_err());
        records[1]["fields"]["libraries"] = json!(7);
        let expected = verify(&output(&records), 1, 2, 1, true).unwrap();
        records[1]["fields"]["libraries"] = json!(6);
        let actual = verify(&output(&records), 1, 2, 1, true).unwrap();
        assert!(verify_expected(&actual, &expected).is_err());
        assert!(verify(&output(&records), 1, 2, 1, false).is_err());
    }
    #[test]
    fn rejects_missing_work_skipped_inputs_and_failures() {
        let lines = records()
            .iter()
            .map(Value::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(verify(&captured(&lines), 1, 2, 1, false).is_ok());
        let mut failed_exit = captured(&lines);
        failed_exit.status = Command::new("false").status().unwrap();
        assert!(
            verify(&failed_exit, 1, 2, 1, false)
                .unwrap_err()
                .to_string()
                .contains("process exit")
        );
        for rule in [
            "syntax-error",
            "internal-error",
            "rules-thread-error",
            "config",
            "unmatched-path",
            "incomplete-analysis",
        ] {
            let mut output = captured(&lines);
            output.stdout = serde_json::to_vec(&json!([{"rule":rule}])).unwrap();
            assert!(
                verify(&output, 1, 2, 1, false)
                    .unwrap_err()
                    .to_string()
                    .contains("failure diagnostic")
            );
        }
        for level in ["WARN", "ERROR"] {
            let output = captured(&format!(
                "{lines}\n{}",
                json!({"level":level,"fields":{"message":"failed to read input"}})
            ));
            assert!(
                verify(&output, 1, 2, 1, false)
                    .unwrap_err()
                    .to_string()
                    .contains("warning/failure")
            );
        }
        for count in [Value::Null, json!(1)] {
            let mut degraded = records();
            degraded[1]["fields"]["parse_error_files"] = count;
            let output = captured(
                &degraded
                    .iter()
                    .map(Value::to_string)
                    .collect::<Vec<_>>()
                    .join("\n"),
            );
            assert!(verify(&output, 1, 2, 1, false).is_err());
        }
        let mut external = records();
        external[1]["fields"]["libraries"] = json!(1);
        let output = captured(
            &external
                .iter()
                .map(Value::to_string)
                .collect::<Vec<_>>()
                .join("\n"),
        );
        assert!(
            verify(&output, 1, 2, 1, false)
                .unwrap_err()
                .to_string()
                .contains("work mismatch")
        );
        assert!(
            verify(&captured(&lines), 2, 2, 1, false)
                .unwrap_err()
                .to_string()
                .contains("work mismatch")
        );
    }
    #[test]
    fn expected_work_detects_noop_rules_and_lost_resolution_findings() {
        let lines = records()
            .iter()
            .map(Value::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        let mut output = captured(&lines);
        output.stdout = br#"[{"rule":"cross-rule"},{"rule":"resolved-rule"}]"#.to_vec();
        let expected = verify(&output, 1, 2, 1, false).unwrap();
        assert!(verify_expected(&expected, &expected).is_ok());
        for finding in ["cross-rule", "resolved-rule"] {
            output.stdout = serde_json::to_vec(&json!([{"rule":finding}])).unwrap();
            let actual = verify(&output, 1, 2, 1, false).unwrap();
            assert!(verify_expected(&actual, &expected).is_err());
        }
    }
    #[test]
    fn work_mismatch_reports_counter_and_per_rule_deltas() {
        let actual = json!({"files":2,"findingsByRule":{"added":2,"changed":1}});
        let expected = json!({"files":1,"findingsByRule":{"removed":3,"changed":4}});
        let error = verify_expected(&actual, &expected).unwrap_err();
        let mismatch = error.downcast_ref::<WorkMismatch>().unwrap();
        assert_eq!(
            mismatch.0["files"],
            json!({"actual":2,"expected":1,"delta":1})
        );
        assert_eq!(mismatch.0["findingsByRule"]["added"]["delta"], 2);
        assert_eq!(mismatch.0["findingsByRule"]["removed"]["delta"], -3);
        assert_eq!(mismatch.0["findingsByRule"]["changed"]["delta"], -3);
        assert!(error.to_string().contains("\"removed\""));
    }

    #[test]
    fn malformed_expectations_return_errors() {
        for source in [
            "invalid",
            "null",
            "[]",
            "1",
            r#"{"case":null}"#,
            r#"{"case":[]}"#,
        ] {
            assert!(expectations(source.as_bytes()).is_err(), "{source}");
        }
        assert!(expectations(br#"{"case":{"files":1}}"#).is_ok());
    }

    #[test]
    fn clean_checkout_rejects_modified_untracked_and_ignored_files() {
        assert!(verify_clean_checkout("").is_ok());
        for status in [" M lib/a.dart\n", "?? new.dart\n", "!! .dart_tool/\n"] {
            assert!(verify_clean_checkout(status).is_err(), "{status}");
        }
    }
}
