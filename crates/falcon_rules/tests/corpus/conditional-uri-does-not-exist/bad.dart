import 'targets/stub.dart'
    if (dart.library.io) 'targets/missing_io.dart' /* expect: conditional-uri-does-not-exist */
    if (dart.library.js_interop) 'targets/web.dart';
import 'targets/stub.dart'
    if (dart.library.io) 'targets/io.dart'
    if (dart.library.js_interop) 'targets/missing_web.dart' /* expect: conditional-uri-does-not-exist */
    if (dart.library.html) 'missing_html.dart' /* expect: conditional-uri-does-not-exist */
    as second;
import 'targets/stub.dart'
    if (app.flavor == 'prod') 'targets/prod.dart' /* expect: conditional-uri-does-not-exist */
    as third;
export 'targets/stub.dart'
    if (dart.library.io) 'targets/missing_export.dart'; /* expect: conditional-uri-does-not-exist */
import 'targets/stub.dart'
    if (dart.library.io) "targets/io" /* expect: conditional-uri-does-not-exist */
    if (dart.library.ffi) r'targets/raw_missing.dart' /* expect: conditional-uri-does-not-exist */
    as fourth;

void main() {}
