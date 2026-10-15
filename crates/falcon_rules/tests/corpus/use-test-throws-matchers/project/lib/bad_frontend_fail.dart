import 'src/frontend/expect.dart';
import 'src/frontend/expect.dart' as expectations;

int parse(String input) => int.parse(input);

void throwsOnEmptyInput() {
  try {
    parse('');
    fail('Expected a FormatException'); /* expect: use-test-throws-matchers */
  } on FormatException {}
}

void catchAll() {
  try {
    parse('x');
    expectations.fail('no throw'); /* expect: use-test-throws-matchers */
  } catch (e) {
    print(e);
  }
}

void twoCatchClauses() {
  try {
    parse('');
    fail('no throw');
  } on FormatException {
    // expected
  } on ArgumentError {
    // also fine
  }
}

void localFailShadows() {
  void fail(String message) => print(message);
  try {
    parse('');
    fail('no throw');
  } catch (_) {}
}
