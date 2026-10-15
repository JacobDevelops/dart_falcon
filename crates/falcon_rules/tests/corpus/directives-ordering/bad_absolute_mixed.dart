// An absolute path causes first-segment comparison, even with a relative URI.
import '-x/a.dart';
import '/abs/x.dart'; /* expect: directives-ordering */
