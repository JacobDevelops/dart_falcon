use jdlint_analyze::{AnalyzeContext, Rule};
use jdlint_diagnostics::{Diagnostic, Severity, Span as DiagSpan};
use jdlint_syntax::ast::*;

pub struct UnnecessaryFlutterImports;

impl Rule for UnnecessaryFlutterImports {
    fn name(&self) -> &'static str {
        "unnecessary_flutter_imports"
    }

    fn analyze(&self, program: &Program, ctx: &AnalyzeContext) -> Vec<Diagnostic> {
        let mut diags = Vec::new();

        // Check if any Flutter symbols are actually used in the source
        let has_flutter_usage = has_flutter_usage(ctx.source);

        // Flag all Flutter and dart:async imports if they're not being used
        for import in &program.imports {
            if is_unnecessary_import(&import.uri.value) && !has_flutter_usage {
                diags.push(Diagnostic::new(
                    "unnecessary_flutter_imports",
                    Severity::Warning,
                    "Unnecessary Flutter import. Remove unused imports.",
                    ctx.file_path.to_string_lossy().into_owned(),
                    DiagSpan {
                        start: import.span.start,
                        end: import.span.end,
                    },
                ));
            }
        }

        diags
    }
}

fn is_unnecessary_import(uri: &str) -> bool {
    uri.contains("package:flutter") || uri.contains("dart:async")
}

fn has_flutter_usage(source: &str) -> bool {
    // Check for common Flutter symbols that indicate real usage
    let flutter_symbols = [
        "runApp",
        "StatelessWidget",
        "StatefulWidget",
        "MaterialApp",
        "Scaffold",
        "AppBar",
        "Center",
        "Text",
        "Widget",
        "BuildContext",
        "State",
        "Future",
        "Stream",
        "async",
        "await",
        "debugPrint",
        "Color",
        "EdgeInsets",
        "SizedBox",
        "Column",
        "Row",
        "Container",
        "FloatingActionButton",
        "ElevatedButton",
        "TextField",
        "ListView",
        "GridView",
    ];

    let source_without_imports = remove_import_lines(source);

    for &symbol in &flutter_symbols {
        if source_without_imports.contains(symbol) {
            return true;
        }
    }

    // Also check if any symbols are preceded by `extends ` or `implements ` or other type usage
    if source_without_imports.contains("extends StatelessWidget")
        || source_without_imports.contains("extends StatefulWidget")
        || source_without_imports.contains("implements Widget")
        || source_without_imports.contains("const MyApp")
        || source_without_imports.contains("async ")
    {
        return true;
    }

    false
}

fn remove_import_lines(source: &str) -> String {
    let mut result = String::new();
    for line in source.lines() {
        let trimmed = line.trim_start();
        if !trimmed.starts_with("import ") && !trimmed.starts_with("export ") {
            result.push_str(line);
            result.push('\n');
        }
    }
    result
}
