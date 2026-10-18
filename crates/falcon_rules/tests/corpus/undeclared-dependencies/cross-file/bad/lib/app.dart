import 'package:missing_0/api.dart'; /* expect: undeclared-dependencies */
import 'package:missing_1/api.dart'; /* expect: undeclared-dependencies */
import 'package:missing_2/api.dart'; /* expect: undeclared-dependencies */
import 'package:missing_3/api.dart'; /* expect: undeclared-dependencies */
export 'package:test_helpers/api.dart'; /* expect: undeclared-dependencies */
import 'local.dart' if (dart.library.io) 'package:conditional/api.dart'; /* expect: undeclared-dependencies */
export 'local.dart' if (dart.library.html) 'package:conditional_export/api.dart'; /* expect: undeclared-dependencies */
