import 'package:test/test.dart' as t;
void main() {
  t.test('good', () async {
    await t.expectLater(Future.value(1), t.completes);
    await t.expectLater(Stream.value(1), t.emits(1));
    t.expect(1, t.equals(1));
    t.expect(true, t.isTrue);
    t.expect(null, t.isNull);
    final completes = 1;
    t.expect(1, completes);
    void expect(dynamic value, dynamic matcher) {}
    expect(Future.value(1), t.completes);
  });
}
