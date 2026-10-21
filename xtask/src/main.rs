use clap::{Parser, Subcommand};
use falcon_rules::meta::RuleGroup;
use std::path::PathBuf;

mod bench_compare;
mod codegen;
mod docgen;
mod perf_lock;
mod schema;
mod support;
mod validate_rules;
mod version;

#[path = "../../crates/falcon_rules/tests/support/corpus.rs"]
mod corpus;

#[derive(Parser)]
#[command(name = "cargo xtask")]
struct Cli {
    #[command(subcommand)]
    task: Task,
}

#[derive(Subcommand)]
enum Task {
    /// Generate rule implementation and fixture stubs.
    Codegen {
        #[command(subcommand)]
        target: Codegen,
    },
    /// Validate rule implementations against the golden corpus.
    ValidateRules(validate_rules::Options),
    /// Enforce the performance budget on the jfit mobile library.
    PerfLock(perf_lock::Options),
    /// Compare release falcon with dart analyze on pinned public projects.
    BenchCompare(bench_compare::Options),
    /// Generate the configuration schema.
    Schema {
        #[arg(long)]
        check: bool,
    },
    /// Generate the documentation website's data.
    Docgen {
        /// Fail if committed data differs, without writing files.
        #[arg(long)]
        check: bool,
        /// Use an already-built falcon binary instead of building one.
        #[arg(long)]
        falcon_bin: Option<PathBuf>,
    },
    /// Set the workspace version and every pinned reference to it.
    Version {
        /// The new version, e.g. 0.4.2.
        version: String,
    },
}

#[derive(Subcommand)]
enum Codegen {
    Rule {
        #[arg(long, value_parser = clap::builder::PossibleValuesParser::new(RuleGroup::ALL.iter().map(|group| group.as_str())))]
        group: String,
        #[arg(long, help = "Canonical kebab-case rule id", value_parser = codegen::validate_rule_id)]
        name: String,
        #[arg(long, help = "Generate a project-wide CrossFileRule")]
        cross_file: bool,
    },
}

fn main() {
    let result = match Cli::parse().task {
        Task::Codegen {
            target:
                Codegen::Rule {
                    group,
                    name,
                    cross_file,
                },
        } => codegen::run(group, name, cross_file),
        Task::ValidateRules(options) => validate_rules::run(options),
        Task::PerfLock(options) => {
            perf_lock::run(options);
            Ok(())
        }
        Task::BenchCompare(options) => {
            bench_compare::run(&options).map_err(|error| error.to_string())
        }
        Task::Schema { check } => {
            schema::run(check);
            Ok(())
        }
        Task::Docgen { check, falcon_bin } => docgen::run(check, falcon_bin),
        Task::Version { version } => {
            version::run(&version);
            Ok(())
        }
    };
    if let Err(error) = result {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn workspace_root() -> PathBuf {
    // When run via `cargo xtask`, CWD is the workspace root.
    std::env::current_dir().unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codegen_cli_uses_rule_id_value_parser() {
        assert!(
            Cli::try_parse_from([
                "xtask",
                "codegen",
                "rule",
                "--group",
                "correctness",
                "--name",
                "avoid_foo",
            ])
            .is_err()
        );
    }
}
