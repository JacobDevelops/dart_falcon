use std::process::Command;

fn check_chain(parameter_type: &str, value: &str, operand: &str, operator: &str, form: &str) {
    let project = tempfile::tempdir().unwrap();
    std::fs::write(project.path().join("falcon.json"), "{}\n").unwrap();
    let expression = format!("a{}", format!(" {operator} {operand}").repeat(99_999));
    let declaration = match form {
        "initializer" => format!("final x = {expression}; print(x);"),
        "argument" => format!("consume({expression});"),
        "statement" => format!("{expression};"),
        "top_level" => format!("final x = {expression};"),
        "field" => format!("class C {{ final x = {expression}; }}"),
        _ => unreachable!(),
    };
    let body = if matches!(form, "top_level" | "field") {
        declaration
    } else {
        format!("void f({parameter_type} a) {{ {declaration} }}")
    };
    let source = format!(
        "void marker(Object markerParameter) {{}}\nvoid consume(Object? value) {{ print(value); }}\n{parameter_type} a = {value};\n{body}\n"
    );
    std::fs::write(project.path().join("main.dart"), source).unwrap();
    // Subprocesses use normal stacks independently of the test harness.
    let output = Command::new(env!("CARGO_BIN_EXE_falcon"))
        .args([
            "check",
            ".",
            "--format",
            "json",
            "--max-errors",
            "1",
            "--error-on-warnings",
            "--exit-code",
            "2",
        ])
        .current_dir(project.path())
        .env_remove("DART_SDK")
        .env_remove("FLUTTER_ROOT")
        .env_remove("RUST_MIN_STACK")
        .env("PATH", "")
        .output()
        .unwrap();
    assert!(
        matches!(output.status.code(), Some(2)),
        "{form}: {}\nstdout: {}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    // A known lint proves file discovery; exit 2 excludes worker and pipeline failures.
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("avoid-unused-parameters")
            && stdout.contains("markerParameter")
            && !stdout.contains("rules-thread-error"),
        "{form}: analysis did not complete\nstdout: {stdout}\nstderr: {}",
        String::from_utf8_lossy(&output.stderr),
    );
}

macro_rules! chain_tests {
    ($module:ident, $parameter_type:literal, $value:literal, $operand:literal, $operator:literal; $($form:ident),*) => {
        mod $module {
            $(
                #[test]
                fn $form() {
                    super::check_chain($parameter_type, $value, $operand, $operator, stringify!($form));
                }
            )*
        }
    };
}

chain_tests!(addition, "int", "1", "a", "+"; initializer, argument, statement, top_level, field);
chain_tests!(and, "bool", "true", "a", "&&"; initializer, argument, statement, top_level, field);
chain_tests!(or, "bool", "true", "a", "||"; initializer, argument, statement, top_level, field);
chain_tests!(if_null, "int?", "1", "a", "??"; initializer, argument, statement, top_level, field);
chain_tests!(string, "String", "'x'", "'x'", "+"; initializer, argument, statement, top_level, field);
