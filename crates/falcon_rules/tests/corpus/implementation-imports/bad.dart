// No expectations on purpose: with no enclosing pubspec.yaml the rule cannot
// name the source package, so it reports nothing. project/lib/bad_main.dart
// covers the reporting case.
import 'package:foo/src/internal.dart';import 'package:bar/src/helper.dart';import 'package:baz/src/util/log.dart';import 'package:collection/src/list.dart';import 'package:http/src/client.dart';
void main() {}
