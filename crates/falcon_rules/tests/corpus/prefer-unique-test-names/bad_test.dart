import 'package:test/test.dart';
void main() {
  test('same', () {});
  test('same', () {}); /* expect: prefer-unique-test-names */
  test("same", () {}); /* expect: prefer-unique-test-names */
  group('inner', () {
    test('duplicate', () {});
    test('duplicate', () {}); /* expect: prefer-unique-test-names */
    test('duplicate', () {}); /* expect: prefer-unique-test-names */
  });
  const description = 'same';
  test(description, () {}); /* expect: prefer-unique-test-names */
}
