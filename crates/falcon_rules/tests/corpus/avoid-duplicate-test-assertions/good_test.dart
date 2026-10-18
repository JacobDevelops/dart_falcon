import 'package:test/test.dart';
void change() {}
int next() => 1;
class Checker { void expect(Object? a, Object? b) {} }
void main() {
  test('assignment resets', () {
    var value = 1;
    expect(value, 1);
    value = 2;
    expect(value, 1);
  });
  test('call resets', () { expect(1, 1); change(); expect(1, 1); });
  test('branches', () { if (next() == 1) { expect(1, 1); } else { expect(1, 1); } });
  test('first test', () { expect(1, 1); });
  test('second test', () { expect(1, 1); });
  test('different matcher', () { expect(1, 1); expect(1, 2); });
  test('effectful actual', () { expect(next(), 1); expect(next(), 1); });
  test('local shadow', () { void expect(Object? a, Object? b) {} expect(1, 1); expect(1, 1); });
  test('object member', () { final c = Checker(); c.expect(1, 1); c.expect(1, 1); });
  test('await boundary', () async { await expectLater(1, 1); await expectLater(1, 1); });
  test('different reason', () { expect(1, 1, reason: 'a'); expect(1, 1, reason: 'b'); });
}
void matcherBoundaries() {
  test('effectful matcher', () {
    expect(1, equals(next()));
    expect(1, equals(next()));
  });
  test('interpolated actual', () {
    expect('${next()}', 1);
    expect('${next()}', 1);
  });
  test('shadowed matcher factory', () {
    Object? equals(Object? value) => value;
    expect(1, equals(1));
    expect(1, equals(1));
  });
}

void asynchronousBoundaries(Object queue, Object eventMatcher) {
  test('stateful asynchronous matcher', () {
    expectLater(queue, eventMatcher);
    expectLater(queue, eventMatcher);
  });
  test('chained side effect breaks run', () {
    expectLater(1, 1);
    expectLater(1, 1).then((_) { next(); });
    expectLater(1, 1);
  });
}
