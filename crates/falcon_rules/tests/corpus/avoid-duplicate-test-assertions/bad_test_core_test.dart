import 'package:test_core/test_core.dart';

void main() {
  test('direct import enables duplicate detection', () {
    expect(1, 1);
    expect(1, 1); /* expect: avoid-duplicate-test-assertions */
  });
  test('framework matcher constant is inert', () {
    expect(true, isTrue);
    expect(true, isTrue); /* expect: avoid-duplicate-test-assertions */
  });
  test('framework matcher factory is inert', () {
    expect(1, equals(1));
    expect(1, equals(1)); /* expect: avoid-duplicate-test-assertions */
  });
}
