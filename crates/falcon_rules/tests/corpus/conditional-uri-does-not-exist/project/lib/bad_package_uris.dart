import 'src/present.dart'
    if (dart.library.io) 'package:app/src/missing_io.dart' /* expect: conditional-uri-does-not-exist */
    if (dart.library.js_interop) 'package:app/src/present.dart'; /* expect: conditional-uri-does-not-exist */
export 'src/present.dart'
    if (dart.library.io) 'package:app/missing.dart'; /* expect: conditional-uri-does-not-exist */
import 'src/present.dart'
    if (dart.library.io) 'package:app/src/present.dart#fragment'; /* expect: conditional-uri-does-not-exist */
import 'src/present.dart'
    if (dart.library.io) 'package:other/missing.dart'; /* expect: conditional-uri-does-not-exist */
// The pubspec's name does not register app: the package configuration names this
// library root configured_app, so package:app remains unresolved.
import 'src/present.dart'
    if (dart.library.io) 'package:app/src/present.dart'; /* expect: conditional-uri-does-not-exist */
import 'src/present.dart'
    if (dart.library.io) 'package:app/src/present.dart?query'; /* expect: conditional-uri-does-not-exist */
