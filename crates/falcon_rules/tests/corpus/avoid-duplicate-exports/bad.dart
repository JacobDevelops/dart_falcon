export 'a.dart';
export 'a.dart'; /* expect: avoid-duplicate-exports */
export 'b.dart';
export 'b.dart' show B; /* expect: avoid-duplicate-exports */
export 'c.dart' show C, D;
export 'c.dart' show C; /* expect: avoid-duplicate-exports */
export 'd.dart' hide D;
export 'd.dart' show E; /* expect: avoid-duplicate-exports */
export 'e.dart' hide E;
export 'e.dart' hide E, F; /* expect: avoid-duplicate-exports */
export 'f.dart' show F; /* expect: avoid-duplicate-exports */
export 'f.dart';
export 'g.dart' if (dart.library.io) 'io.dart';
export 'g.dart' if (dart.library.io) 'io.dart' show G; /* expect: avoid-duplicate-exports */
export 'h.dart';
export "h.dart"; /* expect: avoid-duplicate-exports */
export 'relative.dart';
export './relative.dart' show Relative; /* expect: avoid-duplicate-exports */
export 'nested/../relative.dart' show Relative; /* expect: avoid-duplicate-exports */
export 'sub/a.dart';
export './sub/a.dart' show A; /* expect: avoid-duplicate-exports */
