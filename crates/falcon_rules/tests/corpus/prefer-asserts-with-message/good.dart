// Asserts that carry a message.
class Range {
  final int start;
  final int end;
  Range(this.start, this.end) : assert(start <= end, 'start must not exceed end');
  Range.unit(this.start)
      : end = start + 1,
        assert(start >= 0, 'start must be non-negative');
}

void check(int value, List<int> items) {
  assert(value > 0, 'value must be positive');
  assert(items.isNotEmpty, 'items must not be empty');
  assert(() {
    return true;
  }(), 'debug-only check');
  if (value > 10) {
    assert(value < 100, 'value is too large: $value');
  }
}
