import 'package:app/src/a.dart'; /* expect: prefer-relative-imports */
import 'package:app/b.dart' as b; /* expect: prefer-relative-imports */
import 'package:app/src/c.dart' show C; /* expect: prefer-relative-imports */
import 'package:app/src/d.dart' deferred as d; /* expect: prefer-relative-imports */
import "package:app/e.dart" hide E; /* expect: prefer-relative-imports */
import 'package:app/src/f.dart' /* expect: prefer-relative-imports */
    if (dart.library.io) 'src/f_io.dart';

void main() {}
