import 'package:test/test.dart';

void main() {
  test('explicit default', () {
    // arrange
    final a = 1;
    // act
    final b = a + 1;
    // assert
    expect(b, 2);
  });
}
