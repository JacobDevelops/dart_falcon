import '../lib/api.dart';
void checkSamePackage() {
  oldFunction(); /* expect: deprecated-member-use-from-same-package */
}
