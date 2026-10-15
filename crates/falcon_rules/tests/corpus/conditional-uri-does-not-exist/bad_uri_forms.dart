// Analyzer 3.13.5: file URIs need an absolute path; malformed schemes and
// percent escapes still report missing conditional sources.
import 'targets/stub.dart'
    if (dart.library.io) 'file:/falcon-uri-fixture-missing.dart'; /* expect: conditional-uri-does-not-exist */
import 'targets/stub.dart'
    if (dart.library.io) 'file:targets/stub.dart'; /* expect: conditional-uri-does-not-exist */
import 'targets/stub.dart'
    if (dart.library.io) 'file://host/falcon-uri-fixture-missing.dart'; /* expect: conditional-uri-does-not-exist */
import 'targets/stub.dart'
    if (dart.library.io) '1bad:foo'; /* expect: conditional-uri-does-not-exist */
import 'targets/stub.dart'
    if (dart.library.io) '%ZZ.dart'; /* expect: conditional-uri-does-not-exist */
import 'targets/stub.dart'
    if (dart.library.io) r'targets/' '\x6dissing.dart'; /* expect: conditional-uri-does-not-exist */
