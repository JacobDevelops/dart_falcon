import 'package:test/test.dart' as t;
void main() {
  t.test('dedicated matchers', () {
    t.expect(true, t.isTrue);
    t.expect(false, t.isFalse);
    t.expect(null, t.isNull);
    t.expect(1, t.equals(1));
    t.expect([1], t.isNotNull);
    void expect(dynamic actual, dynamic matcher) {}
    expect(true, true);
  });
}
