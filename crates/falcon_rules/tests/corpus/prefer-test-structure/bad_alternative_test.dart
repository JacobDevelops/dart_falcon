import 'package:test/test.dart';

void main() {
  test('default markers are disabled', () { /* expect: prefer-test-structure */
    // arrange
    final a = 1;
    // act
    final b = a + 1;
    // assert
    expect(b, 2);
  });
  test('missing given', () {
    // when
    final a = 1; /* expect: prefer-test-structure, msg: "This test is missing the `given` section before this `when` section. Try refactoring this test." */
    // then
    expect(a, 1);
  });
  test('missing when', () {
    // given
    final a = 1;
    // then
    expect(a, 1); /* expect: prefer-test-structure */
  });
  test('missing then', () { /* expect: prefer-test-structure */
    // given
    final a = 1;
    // when
    final b = a + 1;
  });
}
