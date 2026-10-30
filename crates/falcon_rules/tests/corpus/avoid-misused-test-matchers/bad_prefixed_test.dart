import 'package:test/test.dart' as t;
void main() async {
  t.expect(1, t.isTrue); /* expect: avoid-misused-test-matchers */
  await t.expectLater(false, t.isZero); /* expect: avoid-misused-test-matchers */
  t.expect('text', t.isA<int>()); /* expect: avoid-misused-test-matchers */
  t.expect({'a': 1}, t.isList); /* expect: avoid-misused-test-matchers */
}
