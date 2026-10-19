import 'package:test/test.dart';
void testCase(String name, dynamic body) { test(name, body); }
void main() {
  testCase('same custom name', () {});
  testCase('same custom name', () {}); /* expect: prefer-unique-test-names */
  for (final value in [1, 2]) {
    testCase('constant custom name', () {}); /* expect: prefer-unique-test-names */
  }
}
