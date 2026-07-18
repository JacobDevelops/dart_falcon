// Test cases for avoid-ignoring-return-values rule — no violations
// Return values are used or calls are void-typed.

String transform(String input) => input.toUpperCase();

int compute(int x, int y) => x + y;

List<int> buildList() => [1, 2, 3];

void goodUsage() {
  final result = transform('hello');
  final sum = compute(2, 3);
  final list = buildList();
  final upper = 'hello'.toUpperCase();
  final doubled = [1, 2, 3].map((x) => x * 2).toList();
  print('$result $sum $list $upper $doubled');
}

void voidCallsAreOk() {
  print('logging is fine');
  [1, 2, 3].forEach(print);
}

// `configure` is NOT on the side-effect allowlist, but the project index proves
// it returns void, so discarding its result is correctly not flagged — the
// resolver removes a false positive the allowlist would have raised.
void configure() {}

void resolvedVoidDiscard() {
  configure();
}

class Counter {
  int value = 0;

  int increment() => ++value;

  void goodMethodCall() {
    final newValue = increment();
    print(newValue);
  }
}
