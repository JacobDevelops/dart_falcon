import 'support.dart';
const value = 1; /* expect: avoid-top-level-members-in-tests */
var count = 0; /* expect: avoid-top-level-members-in-tests */
final values = <int>[]; /* expect: avoid-top-level-members-in-tests */
void helper() {} /* expect: avoid-top-level-members-in-tests */
class Helper {} /* expect: avoid-top-level-members-in-tests */
void main() { registerTests(); }
