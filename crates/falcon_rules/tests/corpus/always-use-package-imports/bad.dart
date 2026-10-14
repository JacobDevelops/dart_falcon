// No expectations on purpose: this file has no enclosing pubspec.yaml, so it
// is not in a package's lib/ directory and the rule stays silent, like the
// analyzer's `isInLibDir`. project/lib/bad_imports.dart and
// project/lib/src/bad_nested.dart cover the reporting cases.
import 'src/a.dart';
import '../b.dart' as b;

void main() {}
