// Bad: using [0] instead of .first
void example() {
  final items = [1, 2, 3];
  final first = items[0]; /* expect: prefer-first */
  print(first);
}

class Processor {
  String getFirstName(List<String> names) {
    return names[0]; /* expect: prefer-first */
  }

  void processHead(List<int> values) {
    final head = values[0]; /* expect: prefer-first */
    if (head > 0) {
      compute(head);
    }
  }
}

void multipleViolations(List<String> items) {
  final a = items[0]; /* expect: prefer-first */
  final b = items[0].length; /* expect: prefer-first */
  print('$a $b');
}
