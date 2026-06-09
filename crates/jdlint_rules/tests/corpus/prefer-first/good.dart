// Good: using .first
void example() {
  final items = [1, 2, 3];
  final first = items.first;
  print(first);
}

class Processor {
  String getFirstName(List<String> names) {
    return names.first;
  }

  void processHead(List<int> values) {
    final head = values.first;
    if (head > 0) {
      compute(head);
    }
  }
}

void multipleAccess(List<String> items) {
  final a = items.first;
  final b = items.first.length;
  print('$a $b');
}

// OK: accessing non-zero indices
void accessOther(List<int> items) {
  final second = items[1];
  final third = items[2];
}

// OK: in loop context
void loopAccess(List<List<int>> matrix) {
  for (int i = 0; i < matrix.length; i++) {
    final row = matrix[i]; // accessing by variable index is OK
  }
}
