import 'package:test/test.dart';
import 'package:test/test.dart' as t;

int parse(String input) => int.parse(input);

void main() {
  test('throws on empty input', () {
    try {
      parse('');
      fail('Expected a FormatException'); /* expect: use-test-throws-matchers */
    } on FormatException {}
  });

  test('catch all', () {
    try {
      parse('x');
      fail('no throw'); /* expect: use-test-throws-matchers */
    } catch (e) {
      expect(e, isA<FormatException>());
    }
  });

  test('prefixed fail', () {
    try {
      parse('y');
      t.fail('no throw'); /* expect: use-test-throws-matchers */
    } on FormatException catch (e) {
      expect(e.message, isNotEmpty);
    }
  });

  test('only fail', () {
    try {
      fail('always'); /* expect: use-test-throws-matchers */
    } catch (_) {}
  });

  test('nested', () {
    for (final input in ['', 'x']) {
      try {
        parse(input);
        fail('no throw for $input'); /* expect: use-test-throws-matchers */
      } on FormatException {}
    }
  });
}
