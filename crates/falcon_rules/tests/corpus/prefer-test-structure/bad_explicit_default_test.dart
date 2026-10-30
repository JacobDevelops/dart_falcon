import 'package:test/test.dart';

void main() {
  test('alternative markers are disabled', () { /* expect: prefer-test-structure */
    // given
    final a = 1;
    // when
    final b = a + 1;
    // then
    expect(b, 2);
  });
}
