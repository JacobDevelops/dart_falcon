import 'package:test/test.dart';

void main() {
  test('repeated arrange before assertion', () {
    // arrange
    final a = 1;
    // act
    final b = a + 1;
    // arrange
    final c = 3; /* expect: prefer-test-structure */
    // act
    final d = b + c;
    // assert
    expect(d, 5);
  });
  test('trailing comments are not section headers', () { /* expect: prefer-test-structure */
    final a = 1; // arrange
    final b = a + 1; // act
    expect(b, 2); // assert
  });
  test('strings are not comments', () { /* expect: prefer-test-structure */
    final text = '// arrange\n// act\n// assert';
    expect(text, text);
  });
  test('partial words are not headers', () { /* expect: prefer-test-structure */
    // rearrange
    final a = 1;
    // action
    final b = a + 1;
    // assertion
    expect(b, 2);
  });
}
