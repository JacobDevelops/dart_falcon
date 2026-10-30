import 'package:flutter_test/flutter_test.dart';
import 'package:test/test.dart' as tests;

const badName = 'broken';
void main() {
  test('broken', () {}); /* expect: format-test-name, msg: "The `test` description must match `^should .+\.$`." */
  test('', () {}); /* expect: format-test-name */
  tests.test('does not start correctly', () {}); /* expect: format-test-name */
  test(badName, () {}); /* expect: format-test-name */
  test('should ' 'work', () {}); /* expect: format-test-name */
  testWidgets('broken', (tester) {}); /* expect: format-test-name, msg: "The `testWidgets` description must match `^renders .+\.$`." */
  testWidgets('should render.', (tester) {}); /* expect: format-test-name */
  group('not constrained', () {
    test('nested broken', () {}); /* expect: format-test-name */
  });
}
