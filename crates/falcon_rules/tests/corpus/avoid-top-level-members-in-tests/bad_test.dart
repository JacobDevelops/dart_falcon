import 'package:test/test.dart';
var count = 0; /* expect: avoid-top-level-members-in-tests */
const value = 1; /* expect: avoid-top-level-members-in-tests */
final values = <int>[]; /* expect: avoid-top-level-members-in-tests */
void helper() {} /* expect: avoid-top-level-members-in-tests */
class Helper {} /* expect: avoid-top-level-members-in-tests */
mixin Behavior {} /* expect: avoid-top-level-members-in-tests */
enum Choice { first } /* expect: avoid-top-level-members-in-tests */
extension Description on int { String describe() => '$this'; } /* expect: avoid-top-level-members-in-tests */
typedef Callback = void Function(); /* expect: avoid-top-level-members-in-tests */
void main() { test('case', () {}); }
