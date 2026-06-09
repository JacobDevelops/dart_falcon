// Good: descriptive identifier names
void example() {
  var count = compute();
  String text = getText();
  int value = 42;
  List<String> items = [];
}

class Processor {
  String path = '';
  int maxRetries = 0;

  void process(String data) {
    final result = transform(data);
    print(result);
  }
}

// OK: loop variables i, j, k, n are allowed
void goodLoop(List<int> items) {
  for (int i = 0; i < items.length; i++) {
    final value = items[i];
    print(value);
  }
}

void nestedLoops(List<List<int>> matrix) {
  for (int i = 0; i < matrix.length; i++) {
    for (int j = 0; j < matrix[i].length; j++) {
      print(matrix[i][j]);
    }
  }
}

void kLoop(List<String> items) {
  for (int k = 0; k < items.length; k++) {
    print(items[k]);
  }
}

void nLoop(int n) {
  for (int i = 0; i < n; i++) {
    print(i);
  }
}

String formatData(int queryId) {
  return 'Query: $queryId';
}

// OK: conventions like _ for unused
void unused(_) {
  print('Ignored');
}
