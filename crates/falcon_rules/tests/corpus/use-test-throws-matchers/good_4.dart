// `package:test` re-exports `package:matcher`'s `fail`, not one declared in
// `package:test_api/src/frontend/expect.dart`, so the official rule (and
// falcon) report none of these.
import 'package:test/test.dart';
import 'package:test/test.dart' as t;

int parse(String input) => int.parse(input);

void main() {
  test('throws on empty input', () {
    try {
      parse('');
      fail('Expected a FormatException');
    } on FormatException {}
  });

  test('catch all', () {
    try {
      parse('x');
      fail('no throw');
    } catch (e) {
      expect(e, isA<FormatException>());
    }
  });

  test('prefixed fail', () {
    try {
      parse('y');
      t.fail('no throw');
    } on FormatException catch (e) {
      expect(e.message, isNotEmpty);
    }
  });

  test('only fail', () {
    try {
      fail('always');
    } catch (_) {}
  });

  test('nested', () {
    for (final input in ['', 'x']) {
      try {
        parse(input);
        fail('no throw for $input');
      } on FormatException {}
    }
  });
}
