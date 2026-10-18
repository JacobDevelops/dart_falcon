import 'package:test/test.dart';
import 'package:test/test.dart' as t;
void main() {
  test('integer', () {
    expect(1, 1);
    expect(1, 1); /* expect: avoid-duplicate-test-assertions */
  });
  test('local variable', () {
    final value = true;
    expect(value, isTrue);
    expect(value, isTrue); /* expect: avoid-duplicate-test-assertions */
  });
  test('prefixed', () {
    t.expect(false, false);
    t.expect(false, false); /* expect: avoid-duplicate-test-assertions */
  });
  test('later', () {
    expectLater(1, 1);
    expectLater(1, 1); /* expect: avoid-duplicate-test-assertions */
  });
  test('nonadjacent pure assertions', () {
    expect(1, 1);
    expect(2, 2);
    expect(1, 1); /* expect: avoid-duplicate-test-assertions */
  });
  test('three copies', () {
    expect(null, null);
    expect(null, null); /* expect: avoid-duplicate-test-assertions */
    expect(null, null); /* expect: avoid-duplicate-test-assertions */
  });
}
void matcherExamples() {
  test('string matcher factory', () {
    expect('value', equals('value'));
    expect('value', equals('value')); /* expect: avoid-duplicate-test-assertions */
  });
}
