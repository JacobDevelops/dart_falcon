import 'package:test/test.dart';
const constantName = 'constant';
void main() {
  for (final value in [1, 2]) {
    test('fixed name', () {}); /* expect: prefer-unique-test-names */
    test(constantName, () {}); /* expect: prefer-unique-test-names */
  }
  for (var index = 0; index < 2; index++) {
    test('fixed counted loop', () {}); /* expect: prefer-unique-test-names */
  }
}
