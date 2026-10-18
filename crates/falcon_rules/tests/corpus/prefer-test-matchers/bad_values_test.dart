import 'package:test/test.dart';
void main() {
  test('explicit matchers', () {
    expect(1, 1); /* expect: prefer-test-matchers */
    expect('value', 'value'); /* expect: prefer-test-matchers */
    expect([1], [1]); /* expect: prefer-test-matchers */
    expect({'a': 1}, {'a': 1}); /* expect: prefer-test-matchers */
    final expected = 3;
    expect(3, expected); /* expect: prefer-test-matchers */
    expect(1 is int, isTrue); /* expect: prefer-test-matchers */
    expect('a' is! int, isFalse); /* expect: prefer-test-matchers */
  });
}
