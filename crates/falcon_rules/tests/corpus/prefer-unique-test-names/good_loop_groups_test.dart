import 'package:test/test.dart';
void main() {
  for (final item in ['first', 'second']) {
    group('$item', () {
      test('works', () {});
    });
  }
  for (final section in ['left', 'right']) {
    group('section $section', () {
      for (final item in [1, 2]) {
        test('item $item', () {});
      }
    });
  }
}
