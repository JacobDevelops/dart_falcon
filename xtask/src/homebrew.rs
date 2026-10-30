use clap::Args;
use std::{collections::HashMap, fmt::Write, fs, path::PathBuf};

#[derive(Args)]
pub(crate) struct Options {
    /// Release version without the v prefix, e.g. 0.4.2.
    #[arg(long)]
    pub(crate) version: String,
    /// SHA256SUMS from the release containing all three versioned archives.
    #[arg(long)]
    pub(crate) checksums: PathBuf,
    /// Destination formula file, e.g. dist/falcon.rb.
    #[arg(long)]
    pub(crate) output: PathBuf,
}

pub(crate) fn run(options: &Options) -> Result<(), String> {
    let sums = fs::read_to_string(&options.checksums)
        .map_err(|error| format!("read {}: {error}", options.checksums.display()))?;
    let formula = generate(&options.version, &sums)?;
    fs::write(&options.output, formula)
        .map_err(|error| format!("write {}: {error}", options.output.display()))
}

fn generate(version: &str, sums: &str) -> Result<String, String> {
    let mut parts = version.split('.');
    if parts.clone().count() != 3
        || parts.any(|part| {
            part.is_empty()
                || !part.bytes().all(|byte| byte.is_ascii_digit())
                || (part.len() > 1 && part.starts_with('0'))
        })
    {
        return Err(format!("expected a version like 0.4.2, got `{version}`"));
    }

    let mut checksums = HashMap::with_capacity(sums.lines().count());
    for (index, line) in sums.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let mut fields = line.split_whitespace();
        let checksum = fields.next().unwrap();
        let asset = fields.next();
        if asset.is_none()
            || fields.next().is_some()
            || checksum.len() != 64
            || !checksum.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(format!("invalid SHA256SUMS entry on line {}", index + 1));
        }
        let asset = asset.unwrap();
        let asset = asset.strip_prefix('*').unwrap_or(asset);
        if checksums.insert(asset, checksum).is_some() {
            return Err(format!("duplicate checksum for {asset}"));
        }
    }

    let mut formula = format!(
        "class Falcon < Formula\n  desc \"Fast Dart linter built in Rust\"\n  homepage \"https://dart-falcon.jacobdevelops.com\"\n  version \"{version}\"\n  license \"MIT\"\n"
    );
    for (os, platform, architectures) in [
        ("macos", "darwin", &[("arm", "aarch64")][..]),
        (
            "linux",
            "linux",
            &[("arm", "aarch64"), ("intel", "x86_64")][..],
        ),
    ] {
        writeln!(formula, "\n  on_{os} do").unwrap();
        for (cpu, architecture) in architectures {
            let asset = format!("falcon-{version}-{architecture}-{platform}.tar.gz");
            let checksum = checksums
                .get(asset.as_str())
                .ok_or_else(|| format!("SHA256SUMS has no checksum for {asset}"))?;
            writeln!(
                formula,
                "    on_{cpu} do\n      url \"https://github.com/JacobDevelops/dart_falcon/releases/download/v{version}/{asset}\"\n      sha256 \"{checksum}\"\n    end"
            )
            .unwrap();
        }
        writeln!(formula, "  end").unwrap();
    }
    formula.push_str(
        "\n  def install\n    bin.install \"falcon\"\n  end\n\n  test do\n    assert_match version.to_s, shell_output(\"#{bin}/falcon --version\")\n  end\nend\n",
    );
    Ok(formula)
}

#[cfg(test)]
mod tests {
    use super::*;

    const VERSION: &str = "9.8.7";
    const PLATFORMS: [&str; 3] = ["aarch64-darwin", "aarch64-linux", "x86_64-linux"];

    fn checksums() -> String {
        let mut checksums = String::new();
        for (index, platform) in PLATFORMS.iter().enumerate() {
            writeln!(
                checksums,
                "{}  falcon-{VERSION}-{platform}.tar.gz",
                (index + 1).to_string().repeat(64)
            )
            .unwrap();
        }
        checksums
    }

    #[test]
    fn generates_complete_formula_from_release_checksums() {
        let directory = tempfile::tempdir().unwrap();
        let sums = directory.path().join("SHA256SUMS");
        let output = directory.path().join("falcon.rb");
        fs::write(&sums, checksums()).unwrap();
        run(&Options {
            version: VERSION.to_owned(),
            checksums: sums,
            output: output.clone(),
        })
        .unwrap();

        let formula = fs::read_to_string(output).unwrap();
        let expected = format!(
            r##"class Falcon < Formula
  desc "Fast Dart linter built in Rust"
  homepage "https://dart-falcon.jacobdevelops.com"
  version "9.8.7"
  license "MIT"

  on_macos do
    on_arm do
      url "https://github.com/JacobDevelops/dart_falcon/releases/download/v9.8.7/falcon-9.8.7-aarch64-darwin.tar.gz"
      sha256 "{}"
    end
  end

  on_linux do
    on_arm do
      url "https://github.com/JacobDevelops/dart_falcon/releases/download/v9.8.7/falcon-9.8.7-aarch64-linux.tar.gz"
      sha256 "{}"
    end
    on_intel do
      url "https://github.com/JacobDevelops/dart_falcon/releases/download/v9.8.7/falcon-9.8.7-x86_64-linux.tar.gz"
      sha256 "{}"
    end
  end

  def install
    bin.install "falcon"
  end

  test do
    assert_match version.to_s, shell_output("#{{bin}}/falcon --version")
  end
end
"##,
            "1".repeat(64),
            "2".repeat(64),
            "3".repeat(64),
        );
        assert_eq!(formula, expected);
    }

    #[test]
    fn accepts_binary_markers_crlf_and_unrelated_release_assets() {
        let sums = format!(
            "\r\n{}{}  falcon-x86_64-linux.tar.gz\r\n",
            checksums().replace("  ", " *").replace('\n', "\r\n"),
            "a".repeat(64)
        );
        assert_eq!(
            generate(VERSION, &sums).unwrap(),
            generate(VERSION, &checksums()).unwrap()
        );
    }

    #[test]
    fn rejects_missing_platform_checksum_without_overwriting_output() {
        let directory = tempfile::tempdir().unwrap();
        let sums = directory.path().join("SHA256SUMS");
        let output = directory.path().join("falcon.rb");
        fs::write(
            &sums,
            checksums().lines().skip(1).collect::<Vec<_>>().join("\n"),
        )
        .unwrap();
        fs::write(&output, "existing formula").unwrap();
        let error = run(&Options {
            version: VERSION.to_owned(),
            checksums: sums,
            output: output.clone(),
        })
        .unwrap_err();
        assert!(
            error.contains("falcon-9.8.7-aarch64-darwin.tar.gz"),
            "{error}"
        );
        assert_eq!(fs::read_to_string(output).unwrap(), "existing formula");
    }

    #[test]
    fn rejects_invalid_versions() {
        for version in [
            "",
            "v9.8.7",
            "9.8",
            "9.8.7.6",
            "9.8.7-beta",
            "09.8.7",
            "9.8.7\n",
            "9.8.7\"; system('bad')",
        ] {
            assert!(generate(version, &checksums()).is_err(), "{version:?}");
        }
    }

    #[test]
    fn rejects_malformed_or_duplicate_checksums() {
        let sums = checksums();
        for invalid in [
            sums.replacen(&"1".repeat(64), &"1".repeat(63), 1),
            sums.replacen(&"1".repeat(64), &"g".repeat(64), 1),
            sums.replacen(".tar.gz", ".tar.gz extra", 1),
            format!("{sums}{}\n", sums.lines().next().unwrap()),
        ] {
            assert!(generate(VERSION, &invalid).is_err(), "{invalid}");
        }
        assert!(generate(VERSION, "").is_err());
        assert!(generate(VERSION, &sums.replace(VERSION, "1.2.3")).is_err());
    }

    #[test]
    fn reports_input_and_output_io_errors() {
        let directory = tempfile::tempdir().unwrap();
        let sums = directory.path().join("SHA256SUMS");
        let options = Options {
            version: VERSION.to_owned(),
            checksums: sums.clone(),
            output: directory.path().to_owned(),
        };
        assert!(run(&options).unwrap_err().contains("read"));
        fs::write(sums, checksums()).unwrap();
        assert!(run(&options).unwrap_err().contains("write"));
    }

    #[test]
    fn binstall_urls_match_every_release_matrix_archive() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap();
        let manifest: toml::Value =
            toml::from_str(&fs::read_to_string(root.join("Cargo.toml")).unwrap()).unwrap();
        let workflow: serde_yaml::Value = serde_yaml::from_str(
            &fs::read_to_string(root.join(".github/workflows/release.yml")).unwrap(),
        )
        .unwrap();
        let metadata = &manifest["package"]["metadata"]["binstall"];
        let repository = manifest["workspace"]["package"]["repository"]
            .as_str()
            .unwrap();
        let version = manifest["workspace"]["package"]["version"]
            .as_str()
            .unwrap();
        let matrix = workflow["jobs"]["build"]["strategy"]["matrix"]["include"]
            .as_sequence()
            .unwrap();
        assert_eq!(matrix.len(), 3);
        assert_eq!(metadata["pkg-fmt"].as_str(), Some("tgz"));
        assert_eq!(metadata["bin-dir"].as_str(), Some("{ bin }"));
        for entry in matrix {
            let target = entry["target"].as_str().unwrap();
            let platform = entry["platform"].as_str().unwrap();
            let template = metadata
                .get("overrides")
                .and_then(|overrides| overrides.get(target))
                .and_then(|overrides| overrides.get("pkg-url"))
                .unwrap_or(&metadata["pkg-url"])
                .as_str()
                .unwrap();
            let url = template
                .replace("{ repo }", repository)
                .replace("{ version }", version)
                .replace("{ target }", target)
                .replace("{ target-arch }", target.split('-').next().unwrap())
                .replace(
                    "{ os-name }",
                    if target.contains("linux") {
                        "linux"
                    } else {
                        "macos"
                    },
                );
            assert_eq!(
                url,
                format!(
                    "{repository}/releases/download/v{version}/falcon-{version}-{platform}.tar.gz"
                ),
                "binstall URL for {target}"
            );
        }
    }
}
