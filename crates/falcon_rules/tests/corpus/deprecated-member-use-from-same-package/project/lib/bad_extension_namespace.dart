import 'good_extension_shadow_api.dart';
import 'good_extension_shadow_api.dart' as prefixed;
class Result {
  void old() {}
}
Result ShadowedExtension(String value) => Result();
void uses() {
  ShadowedExtension('').old();
  prefixed.ShadowedExtension.oldStatic(); /* expect: deprecated-member-use-from-same-package */
  prefixed.ShadowedExtension('').old(); /* expect: deprecated-member-use-from-same-package */
  prefixed.ShadowedExtension.oldPositional(1); /* expect: deprecated-member-use-from-same-package */
  prefixed.ShadowedExtension.oldNamed(value: 1); /* expect: deprecated-member-use-from-same-package */
  prefixed.ShadowedExtension.oldPositional();
  prefixed.ShadowedExtension.oldNamed();
}
