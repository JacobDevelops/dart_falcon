// `fail` here is a project function, not the test framework's.
void fail(String message) => throw StateError(message);

int parse(String input) => int.parse(input);

void run() {
  try {
    parse('');
    fail('no throw');
  } catch (_) {}
}
