use super::{
    BTreeSet, Command, Path, PathBuf, Result, Value, fs, json, output, prepare_checkout,
    require_tool_version, text,
};
use falcon_analyze::PackageConfig;

struct Setup {
    id: &'static str,
    corpus: &'static str,
    subpath: &'static str,
    tool: &'static str,
    lock: &'static str,
    dart_version: &'static str,
    packages: usize,
    files: u64,
    lock_source: &'static str,
    flutter: Option<FlutterPin>,
}

struct FlutterPin {
    version: &'static str,
    framework_revision: &'static str,
    engine_revision: &'static str,
}

// Pinned snapshots preserve published dependencies across SDK-specific resolution.
const SETUPS: &[Setup] = &[
    Setup {
        id: "bloc-dart-resolved",
        corpus: "bloc",
        subpath: "packages/bloc",
        tool: "dart@3.13.6",
        lock: include_str!("../../bench-locks/bloc.lock"),
        dart_version: "3.13.6",
        packages: 56,
        files: 43,
        lock_source: "published website benchmark snapshot",
        flutter: None,
    },
    Setup {
        id: "wonderous-flutter-resolved",
        corpus: "wonderous",
        subpath: ".",
        tool: "flutter@3.47.7-stable",
        lock: include_str!("../../bench-locks/wonderous.lock"),
        dart_version: "3.13.5",
        packages: 134,
        files: 192,
        lock_source: "upstream lock normalized with pinned Flutter SDK",
        flutter: Some(FlutterPin {
            version: "3.47.7",
            framework_revision: "abaf9c523780a608bd46686fd5e53740a07077f8",
            engine_revision: "deb287481e3ce9468f3434937ced4240a70539ca",
        }),
    },
];

fn cache(root: &Path) -> PathBuf {
    root.join("target/bench-corpora/resolved")
}

pub(super) fn cases() -> Vec<super::full_analysis::Case> {
    SETUPS
        .iter()
        .map(|setup| super::full_analysis::Case {
            id: setup.id.to_owned(),
            corpus: setup.corpus.to_owned(),
            subpath: setup.subpath.to_owned(),
            files: setup.files,
        })
        .collect()
}

fn check_flutter_version(version: &Value, setup: &Setup) -> Result<()> {
    let pin = setup.flutter.as_ref().ok_or("missing Flutter SDK pin")?;
    if version["frameworkVersion"] != pin.version
        || version["channel"] != "stable"
        || version["dartSdkVersion"] != setup.dart_version
        || version["frameworkRevision"] != pin.framework_revision
        || version["engineRevision"] != pin.engine_revision
    {
        return Err(format!("unexpected pinned Flutter SDK: {version}").into());
    }
    Ok(())
}

fn sdk(setup: &Setup) -> Result<(PathBuf, PathBuf)> {
    let installed = PathBuf::from(text(Command::new("mise").args(["where", setup.tool]))?);
    let executable = if setup.flutter.is_some() {
        let version: Value =
            serde_json::from_slice(&fs::read(installed.join("bin/cache/flutter.version.json"))?)?;
        check_flutter_version(&version, setup)?;
        installed.join("bin/cache/dart-sdk/bin/dart")
    } else {
        installed.join("bin/dart")
    };
    let executable = fs::canonicalize(executable)?;
    require_tool_version(
        "Dart",
        &text(Command::new(&executable).arg("--version"))?,
        setup.dart_version,
    )?;
    let sdk_root = executable
        .parent()
        .and_then(Path::parent)
        .ok_or("Dart SDK root")?;
    if !sdk_root.join("lib/core/core.dart").is_file() {
        return Err("Dart SDK sources are absent".into());
    }
    let sdk_root = sdk_root.to_path_buf();
    Ok((executable, sdk_root))
}

fn project(root: &Path, setup: &Setup) -> PathBuf {
    cache(root).join(setup.corpus).join(setup.subpath)
}

fn check_lock(project: &Path, setup: &Setup) -> Result<()> {
    if fs::read(project.join("pubspec.lock"))? != setup.lock.as_bytes() {
        return Err(format!(
            "{} dependency lock changed; rerun --setup-resolved",
            setup.id
        )
        .into());
    }
    Ok(())
}

fn check_packages(project: &Path, setup: &Setup) -> Result<()> {
    let config_path = project.join(".dart_tool/package_config.json");
    let config: Value = serde_json::from_slice(&fs::read(&config_path)?)?;
    let parsed = PackageConfig::load(&config_path).ok_or("invalid resolved package config")?;
    if !parsed.is_complete() {
        return Err("incomplete resolved package config".into());
    }
    let packages = config["packages"]
        .as_array()
        .ok_or("missing resolved packages")?;
    if config["configVersion"] != 2
        || packages.len() != setup.packages
        || parsed.packages.len() != setup.packages
        || config["generatorVersion"] != setup.dart_version
    {
        return Err(
            "resolved package count or generating SDK differs from the pinned setup".into(),
        );
    }
    let lock: serde_yaml::Value = serde_yaml::from_str(setup.lock)?;
    let manifest: serde_yaml::Value =
        serde_yaml::from_slice(&fs::read(project.join("pubspec.yaml"))?)?;
    let mut expected_names: BTreeSet<&str> = lock["packages"]
        .as_mapping()
        .ok_or("missing locked packages")?
        .keys()
        .map(|name| {
            name.as_str()
                .ok_or_else(|| "invalid locked package name".into())
        })
        .collect::<Result<_>>()?;
    expected_names.insert(
        manifest["name"]
            .as_str()
            .ok_or("missing project package name")?,
    );
    let mut actual_names = BTreeSet::new();
    for package in packages {
        let name = package["name"].as_str().ok_or("missing package name")?;
        if !actual_names.insert(name) {
            return Err(format!("duplicate resolved package {name}").into());
        }
        let language = package["languageVersion"]
            .as_str()
            .ok_or("missing package language version")?;
        let (major, minor) = language
            .split_once('.')
            .ok_or("invalid package language version")?;
        let (major, minor): (u32, u32) = (major.parse()?, minor.parse()?);
        if major > 3 || (major == 3 && minor > 13) {
            return Err(format!("unsupported package language version {language}").into());
        }
    }
    for package in &parsed.packages {
        if !package.root.is_dir() {
            return Err(format!("missing resolved package root {}", package.root.display()).into());
        }
    }
    if actual_names != expected_names {
        return Err("resolved package names differ from the pinned dependency lock".into());
    }
    Ok(())
}

fn check_revision(root: &Path, setup: &Setup) -> Result<()> {
    let pins: Value = serde_json::from_str(include_str!("../../e2e-corpus.json"))?;
    let expected = pins
        .as_array()
        .ok_or("invalid corpus pins")?
        .iter()
        .find(|pin| pin["id"] == setup.corpus)
        .ok_or("missing resolved corpus pin")?;
    let actual = fs::read_to_string(cache(root).join(setup.corpus).join(".git/HEAD"))?;
    if actual.trim()
        != expected["revision"]
            .as_str()
            .ok_or("missing corpus revision")?
    {
        return Err("resolved corpus revision changed; rerun --setup-resolved".into());
    }
    Ok(())
}

fn check_checkout_status(status: &[u8], allowed_lock: &Path) -> Result<()> {
    for record in status
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
    {
        if record.len() < 4 || record[2] != b' ' {
            return Err("invalid resolved corpus checkout status".into());
        }
        let path = std::str::from_utf8(&record[3..])?;
        if record[..2].iter().any(|code| matches!(*code, b'R' | b'C'))
            || (Path::new(path) != allowed_lock
                && !(record.starts_with(b"!! ")
                    && allowed_generated_path(Path::new(path), allowed_lock)))
        {
            return Err(format!(
                "resolved corpus checkout is modified at {path}; use a clean checkout before --setup-resolved"
            ).into());
        }
    }
    Ok(())
}

fn allowed_generated_path(path: &Path, allowed_lock: &Path) -> bool {
    let project = allowed_lock.parent().unwrap_or_else(|| Path::new(""));
    path.strip_prefix(project).is_ok_and(|relative| {
        matches!(
            relative.to_str(),
            Some(
                ".dart_tool/package_config.json"
                    | ".dart_tool/package_graph.json"
                    | ".dart_tool/falcon-resolved-benchmark.json"
            )
        )
    })
}

fn check_clean_checkout(root: &Path, setup: &Setup) -> Result<()> {
    let checkout = cache(root).join(setup.corpus);
    let status = output(Command::new("git").current_dir(checkout).args([
        "status",
        "--porcelain=v1",
        "-z",
        "--untracked-files=all",
        "--ignored",
    ]))?;
    let allowed_lock = if setup.subpath == "." {
        PathBuf::from("pubspec.lock")
    } else {
        Path::new(setup.subpath).join("pubspec.lock")
    };
    check_checkout_status(&status.stdout, &allowed_lock)
}

fn setup_stamp(project: &Path, setup: &Setup, sdk_root: &Path) -> Result<Value> {
    Ok(json!({
        "case": setup.id,
        "packageConfig": fs::read_to_string(project.join(".dart_tool/package_config.json"))?,
        "sdkRoot": sdk_root,
    }))
}

fn check_stamp(actual: &Value, expected: &Value, case: &str) -> Result<()> {
    if actual != expected {
        return Err(format!("{case} setup changed; rerun --setup-resolved").into());
    }
    Ok(())
}

/// Download and resolve the two pinned workloads, strictly outside measurement.
pub(super) fn setup(root: &Path) -> Result<()> {
    let pins: Value = serde_json::from_str(include_str!("../../e2e-corpus.json"))?;
    for setup in SETUPS {
        if cache(root).join(setup.corpus).join(".git").exists() {
            check_clean_checkout(root, setup)?;
        }
        output(Command::new("mise").args(["install", setup.tool]))?;
        if setup.flutter.is_some() {
            output(Command::new("mise").args([
                "x",
                setup.tool,
                "--",
                "flutter",
                "--version",
                "--machine",
            ]))?;
        }
        let pin = pins
            .as_array()
            .ok_or("invalid corpus pins")?
            .iter()
            .find(|pin| pin["id"] == setup.corpus)
            .ok_or("missing resolved corpus pin")?;
        prepare_checkout(
            &cache(root).join(setup.corpus),
            pin["url"].as_str().ok_or("missing corpus URL")?,
            pin["revision"].as_str().ok_or("missing corpus revision")?,
        )?;
        check_revision(root, setup)?;
        check_clean_checkout(root, setup)?;
        let project = project(root, setup);
        let (dart, sdk_root) = sdk(setup)?;
        fs::write(project.join("pubspec.lock"), setup.lock)?;
        check_lock(&project, setup)?;
        let mut get = Command::new(dart);
        get.current_dir(&project)
            .args(["pub", "get", "--enforce-lockfile"]);
        if setup.flutter.is_some() {
            // Use Flutter's pub environment without its source-mutating project migrations.
            let flutter_root = sdk_root.ancestors().nth(3).ok_or("Flutter SDK root")?;
            get.env("FLUTTER_ROOT", flutter_root);
        }
        output(&mut get)?;
        check_lock(&project, setup)?;
        check_clean_checkout(root, setup)?;
        check_packages(&project, setup)?;
        let stamp = setup_stamp(&project, setup, &sdk_root)?;
        fs::write(
            project.join(".dart_tool/falcon-resolved-benchmark.json"),
            serde_json::to_vec_pretty(&stamp)?,
        )?;
        validate(root, setup.id)?;
    }
    Ok(())
}

/// Refuse incomplete or stale setup; never downloads or runs dependency resolution.
pub(super) fn validate(root: &Path, case: &str) -> Result<(PathBuf, PathBuf)> {
    let setup = SETUPS
        .iter()
        .find(|setup| setup.id == case)
        .ok_or_else(|| format!("unknown resolved workload {case}"))?;
    check_revision(root, setup)?;
    check_clean_checkout(root, setup)?;
    let project = project(root, setup);
    check_lock(&project, setup)?;
    let (_, sdk_root) = sdk(setup)?;
    check_packages(&project, setup)?;
    let stamp: Value = serde_json::from_slice(&fs::read(
        project.join(".dart_tool/falcon-resolved-benchmark.json"),
    )?)?;
    check_stamp(&stamp, &setup_stamp(&project, setup, &sdk_root)?, case)?;
    Ok((project, sdk_root))
}

/// Portable setup provenance for the measured report; no host-specific paths.
pub(super) fn metadata(root: &Path, case: &str) -> Result<Value> {
    let setup = SETUPS
        .iter()
        .find(|setup| setup.id == case)
        .ok_or_else(|| format!("unknown resolved workload {case}"))?;
    let project = project(root, setup);
    let config: Value =
        serde_json::from_slice(&fs::read(project.join(".dart_tool/package_config.json"))?)?;
    Ok(json!({
        "sdkTool": setup.tool,
        "dartVersion": config["generatorVersion"],
        "dependencyLockSnapshot": format!("xtask/bench-locks/{}.lock", setup.corpus),
        "packages": config["packages"].as_array().ok_or("missing resolved packages")?.len(),
        "lockSource": setup.lock_source,
    }))
}

#[cfg(test)]
mod tests {
    use super::{
        Path, SETUPS, Value, cache, check_checkout_status, check_flutter_version, check_lock,
        check_packages, check_revision, fs, json,
    };

    #[test]
    fn missing_package_config_is_rejected() {
        let directory = tempfile::tempdir().unwrap();
        assert!(check_packages(directory.path(), &SETUPS[0]).is_err());
    }

    #[test]
    fn altered_or_missing_lock_is_rejected() {
        let directory = tempfile::tempdir().unwrap();
        let lock = directory.path().join("pubspec.lock");
        assert!(check_lock(directory.path(), &SETUPS[0]).is_err());
        fs::write(&lock, SETUPS[0].lock).unwrap();
        check_lock(directory.path(), &SETUPS[0]).unwrap();
        fs::write(lock, format!("{}# changed\n", SETUPS[0].lock)).unwrap();
        assert!(check_lock(directory.path(), &SETUPS[0]).is_err());
    }

    #[test]
    fn missing_downloaded_package_root_is_rejected() {
        let directory = tempfile::tempdir().unwrap();
        let project = directory.path();
        fs::create_dir(project.join(".dart_tool")).unwrap();
        fs::write(project.join("pubspec.yaml"), "name: fixture\n").unwrap();
        let lock: serde_yaml::Value = serde_yaml::from_str(SETUPS[0].lock).unwrap();
        let mut packages: Vec<Value> = lock["packages"]
            .as_mapping()
            .unwrap()
            .keys()
            .map(|name| {
                json!({
                    "name": name.as_str().unwrap(), "rootUri": "../", "languageVersion": "3.0",
                })
            })
            .collect();
        packages.push(json!({
            "name": "fixture", "rootUri": "../missing-download", "languageVersion": "3.0",
        }));
        fs::write(
            project.join(".dart_tool/package_config.json"),
            serde_json::to_vec(&json!({
                "configVersion": 2, "generatorVersion": "3.13.6", "packages": packages,
            }))
            .unwrap(),
        )
        .unwrap();
        let error = check_packages(project, &SETUPS[0]).unwrap_err().to_string();
        assert!(error.contains("missing resolved package root"), "{error}");
    }

    #[test]
    fn checkout_status_allows_only_the_intended_lock_override() {
        let lock = Path::new("packages/bloc/pubspec.lock");
        check_checkout_status(b"", lock).unwrap();
        check_checkout_status(b"?? packages/bloc/pubspec.lock\0", lock).unwrap();
        check_checkout_status(b" M packages/bloc/pubspec.lock\0", lock).unwrap();
        for status in [
            b" M packages/bloc/lib/bloc.dart\0".as_slice(),
            b"?? packages/bloc/extra.dart\0".as_slice(),
            b"?? packages/bloc/extra.yaml\0".as_slice(),
            b"?? extra.txt\0".as_slice(),
            b"R  packages/bloc/pubspec.lock\0old.lock\0".as_slice(),
            b" M packages/bloc/pubspec.lock\0?? extra.dart\0".as_slice(),
        ] {
            assert!(check_checkout_status(status, lock).is_err());
        }
    }

    #[test]
    fn ignored_generated_metadata_is_allowed_but_source_artifacts_are_rejected() {
        let lock = Path::new("pubspec.lock");
        check_checkout_status(
            b"!! .dart_tool/package_config.json\0!! .dart_tool/package_graph.json\0!! .dart_tool/falcon-resolved-benchmark.json\0",
            lock,
        ).unwrap();
        check_checkout_status(
            b"!! packages/bloc/.dart_tool/package_config.json\0",
            Path::new("packages/bloc/pubspec.lock"),
        )
        .unwrap();
        for status in [
            b"!! .dart_tool/dartpad/web_plugin_registrant.dart\0".as_slice(),
            b"!! .dart_tool/dartpad/\0".as_slice(),
            b"!! .flutter-plugins-dependencies\0".as_slice(),
            b"!! .dart_tool/version\0".as_slice(),
            b"!! build/extra.dart\0".as_slice(),
            b"!! generated/analysis_options.yaml\0".as_slice(),
            b"!! .dart_tool/package_config.json/extra.dart\0".as_slice(),
        ] {
            assert!(check_checkout_status(status, lock).is_err());
        }
    }

    #[test]
    fn flutter_metadata_requires_exact_framework_and_engine_revisions() {
        let setup = &SETUPS[1];
        let pin = setup.flutter.as_ref().unwrap();
        let expected = json!({
            "frameworkVersion": pin.version,
            "channel": "stable",
            "dartSdkVersion": setup.dart_version,
            "frameworkRevision": pin.framework_revision,
            "engineRevision": pin.engine_revision,
        });
        check_flutter_version(&expected, setup).unwrap();
        for field in [
            "frameworkVersion",
            "channel",
            "dartSdkVersion",
            "frameworkRevision",
            "engineRevision",
        ] {
            let mut changed = expected.clone();
            changed[field] = json!("different");
            assert!(check_flutter_version(&changed, setup).is_err(), "{field}");
            changed.as_object_mut().unwrap().remove(field);
            assert!(
                check_flutter_version(&changed, setup).is_err(),
                "missing {field}"
            );
        }
    }

    #[test]
    fn moved_checkout_head_is_rejected() {
        let directory = tempfile::tempdir().unwrap();
        let git = cache(directory.path()).join("bloc/.git");
        fs::create_dir_all(&git).unwrap();
        let pins: Vec<Value> = serde_json::from_str(include_str!("../../e2e-corpus.json")).unwrap();
        let pin = pins.iter().find(|pin| pin["id"] == "bloc").unwrap();
        fs::write(git.join("HEAD"), pin["revision"].as_str().unwrap()).unwrap();
        check_revision(directory.path(), &SETUPS[0]).unwrap();
        fs::write(git.join("HEAD"), "different-revision\n").unwrap();
        assert!(check_revision(directory.path(), &SETUPS[0]).is_err());
    }
}
