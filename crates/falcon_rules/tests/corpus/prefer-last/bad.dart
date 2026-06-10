// Bad: using [list.length - 1] instead of .last
void example() {
  final items = [1, 2, 3];
  final last = items[items.length - 1]; /* expect: prefer-last */
  print(last);
}

class Processor {
  String getLastName(List<String> names) {
    return names[names.length - 1]; /* expect: prefer-last */
  }

  void processTail(List<int> values) {
    final tail = values[values.length - 1]; /* expect: prefer-last */
    if (tail > 0) {
      compute(tail);
    }
  }
}

void multipleViolations(List<String> items) {
  final a = items[items.length - 1]; /* expect: prefer-last */
  final b = items[items.length - 1].length; /* expect: prefer-last */
  print('$a $b');
}
