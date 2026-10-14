import 'src/present.dart'
    if (dart.library.io) 'package:app/src/missing_io.dart' /* expect: conditional-uri-does-not-exist */
    if (dart.library.js_interop) 'package:app/src/present.dart';
export 'src/present.dart'
    if (dart.library.io) 'package:app/missing.dart'; /* expect: conditional-uri-does-not-exist */
