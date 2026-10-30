import 'package:test/test.dart';

void main() {
  test('alternative markers', () {
    // given
    final a = 1;
    // when
    final b = a + 1;
    // then
    expect(b, 2);
  });
  test('alternative block markers', () {
    /* GIVEN */
    final a = 1;
    /* WHEN */
    final b = a + 1;
    /* THEN */
    expect(b, 2);
  });
}
