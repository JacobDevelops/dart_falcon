import 'package:test/test.dart' as t;
void main() {
  t.test('comparisons', () {
    t.expect(1 == 2, t.isFalse); /* expect: prefer-test-matchers */
    t.expect(1 != 2, t.isTrue); /* expect: prefer-test-matchers */
    t.expect(1 < 2, t.isTrue); /* expect: prefer-test-matchers */
    t.expect(2 >= 1, t.isTrue); /* expect: prefer-test-matchers */
    t.expect(null == null, t.isTrue); /* expect: prefer-test-matchers */
  });
}
