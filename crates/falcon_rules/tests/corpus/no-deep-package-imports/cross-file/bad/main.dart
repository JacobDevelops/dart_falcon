import 'package:other/widgets/button.dart'; /* expect: no-deep-package-imports, msg: "Import `package:other/other.dart` instead of reaching into `package:other/widgets/button.dart`." */
import 'package:other/widgets/label.dart'; /* expect: no-deep-package-imports */
import 'package:other/widgets/icon.dart'; /* expect: no-deep-package-imports */
import 'package:other/widgets/card.dart'; /* expect: no-deep-package-imports */
import 'package:other/widgets/private.dart'
    if (dart.library.io) 'package:other/widgets/button.dart'; /* expect: no-deep-package-imports */
import 'package:other/caf%C3%A9.dart'; /* expect: no-deep-package-imports */
void main() {}
