import 'package:test/test.dart';
void main() {
  group('suite', () {
    for (final item in [1, 2]) {
      test('constant within suite', () {}); /* expect: prefer-unique-test-names */
    }
  });
}
