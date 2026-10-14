// Asserts without a message.
class Range {
  final int start;
  final int end;
  Range(this.start, this.end) : assert(start <= end); /* expect: prefer-asserts-with-message */
  Range.unit(this.start)
      : end = start + 1,
        assert(start >= 0); /* expect: prefer-asserts-with-message */
}

void check(int value, List<int> items) {
  assert(value > 0); /* expect: prefer-asserts-with-message */
  assert(items.isNotEmpty); /* expect: prefer-asserts-with-message */
  if (value > 10) {
    assert(value < 100); /* expect: prefer-asserts-with-message */
  }
  final f = () {
    assert(items.length < 5); /* expect: prefer-asserts-with-message */
  };
  f();
}
