import 'package:flutter_test/flutter_test.dart';
import 'package:test/test.dart' as framework;

void main() {
  test('unstructured', () { /* expect: prefer-test-structure */
    expect(1, 1);
  });
  test('DCM out of order', () {
    // act
    final a = 1; /* expect: prefer-test-structure, msg: "This test is missing the `arrange` section before this `act` section. Try refactoring this test." */
    final b = 2;
    // arrange
    final c = a + 1;
    // assert
    expect(b, c); /* expect: prefer-test-structure, msg: "This test is missing the `act` section before this `assert` section. Try refactoring this test." */
  });
  test('missing act', () {
    // arrange
    final a = 1;
    // assert
    expect(a, 1); /* expect: prefer-test-structure */
  });
  test('missing assert', () { /* expect: prefer-test-structure */
    // arrange
    final a = 1;
    // act
    final b = a + 1;
  });
  test('only arrange', () { /* expect: prefer-test-structure */
    // arrange
    final a = 1;
  });
  test('empty', () {}); /* expect: prefer-test-structure */
  framework.test('expression body', () => expect(1, 1)); /* expect: prefer-test-structure */
  testWidgets('widget missing sections', (tester) async { /* expect: prefer-test-structure */
    expect(1, 1);
  });
  test('repeated round', () {
    // arrange
    final a = 1;
    // act
    final b = a + 1;
    // assert
    expect(b, 2);
    // arrange
    final c = 3; /* expect: prefer-test-structure */
    // act
    final d = c + 1;
    // assert
    expect(d, 4);
  });
  test('nested comments do not count', () { /* expect: prefer-test-structure */
    void helper() {
      // arrange
      final a = 1;
      // act
      final b = a + 1;
      // assert
      expect(b, 2);
    }
    helper();
  });
}
