import 'a.dart' as first;
import 'a.dart' as second; /* expect: avoid-duplicate-named-imports */
import 'b.dart';
import 'b.dart' as b; /* expect: avoid-duplicate-named-imports */
import 'c.dart' as c;
import 'c.dart'; /* expect: avoid-duplicate-named-imports */
import 'd.dart' as d show D;
import 'd.dart' as other show D; /* expect: avoid-duplicate-named-imports */
import 'e.dart' deferred as e;
import 'e.dart' deferred as otherE; /* expect: avoid-duplicate-named-imports */
import 'f.dart' if (dart.library.io) 'io.dart' as f;
import 'f.dart' if (dart.library.io) 'io.dart' as otherF; /* expect: avoid-duplicate-named-imports */
