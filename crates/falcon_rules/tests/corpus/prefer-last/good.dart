// Good: using .last
void example() {
  final items = [1, 2, 3];
  final last = items.last;
  print(last);
}

class Processor {
  String getLastName(List<String> names) {
    return names.last;
  }

  void processTail(List<int> values) {
    final tail = values.last;
    if (tail > 0) {
      compute(tail);
    }
  }
}

void multipleAccess(List<String> items) {
  final a = items.last;
  final b = items.last.length;
  print('$a $b');
}

// OK: accessing non-last indices
void accessOther(List<int> items) {
  final secondToLast = items[items.length - 2];
  final third = items[2];
}

// OK: in loop context
void loopAccess(List<List<int>> matrix) {
  for (int i = 0; i < matrix.length; i++) {
    final row = matrix[i]; // accessing by variable index is OK
  }
}
