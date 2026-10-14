import 'package:test/test.dart';

int parse(String input) => int.parse(input);

void main() {
  test('uses throwsA', () {
    expect(() => parse(''), throwsA(isA<FormatException>()));
  });

  test('two catch clauses', () {
    try {
      parse('');
      fail('no throw');
    } on FormatException {
      // expected
    } on ArgumentError {
      // also fine
    }
  });

  test('has finally', () {
    try {
      parse('');
      fail('no throw');
    } catch (_) {
    } finally {
      print('done');
    }
  });

  test('fail is not last', () {
    try {
      fail('no throw');
      parse('');
    } catch (_) {}
  });

  test('local fail shadows the matcher', () {
    void fail(String message) => print(message);
    try {
      parse('');
      fail('no throw');
    } catch (_) {}
  });

  test('try/finally only', () {
    try {
      parse('');
    } finally {
      print('done');
    }
  });
}
