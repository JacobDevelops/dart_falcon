import 'src/a.dart'; /* expect: always-use-package-imports */
import 'b.dart' as b; /* expect: always-use-package-imports */
import 'src/c.dart' show C; /* expect: always-use-package-imports */
import 'src/d.dart' deferred as d; /* expect: always-use-package-imports */
import "./e.dart" hide E; /* expect: always-use-package-imports */
import 'src/f.dart' /* expect: always-use-package-imports */
    if (dart.library.io) 'package:app/src/f_io.dart';
import r'src/g.dart'; /* expect: always-use-package-imports */

void main() {}
