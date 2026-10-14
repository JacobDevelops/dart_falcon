// No expectations on purpose: with no enclosing pubspec.yaml the rule cannot
// name the package, so it reports nothing. project/ covers the reporting
// cases (lib/bad_root.dart, lib/src/bad_nested.dart, tool/bad_tool.dart).
library unprefixed_name;
