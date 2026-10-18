export 'package:legacy/api.dart'; /* expect: avoid-banned-exports */
export 'modern.dart' if (dart.library.io) 'package:legacy/io.dart'; /* expect: avoid-banned-exports */
export 'package:legacy/hidden.dart' hide Hidden; /* expect: avoid-banned-exports */
