import 'package:entrypoint_sample/api.dart'; /* expect: avoid-importing-entrypoint-exports */
import '../api.dart' as api; /* expect: avoid-importing-entrypoint-exports */
import '../api.dart' show Value; /* expect: avoid-importing-entrypoint-exports */
import '../api.dart' deferred as later; /* expect: avoid-importing-entrypoint-exports */
import 'value.dart' if (dart.library.io) '../api.dart'; /* expect: avoid-importing-entrypoint-exports */
