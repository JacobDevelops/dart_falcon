import 'package:test/test.dart' as t;
Future<int> number() async => 1;
void main() {
  t.test('unawaited future actuals', () {
    t.expect(Future.value(1), 1); /* expect: prefer-expect-later */
    t.expect(Future.value(1), t.completes); /* expect: prefer-expect-later */
    t.expect(Future.error('bad'), t.throwsA(t.equals('bad'))); /* expect: prefer-expect-later */
    t.expect(number(), t.equals(1)); /* expect: prefer-expect-later */
    final pending = Future.value(1);
    t.expect(pending, t.equals(1)); /* expect: prefer-expect-later */
    t.expect(Future.value(1), t.predicate((value) => true)); /* expect: prefer-expect-later */
  });
}
