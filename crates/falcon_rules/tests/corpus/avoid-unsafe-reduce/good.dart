class Custom {
  int reduce(int Function(int, int) combine) => 0;
}

int safe(List<int> list, Custom custom, dynamic unknown) {
  custom.reduce((a, b) => a + b);
  unknown.reduce((a, b) => a + b);
  [1, 2].reduce((a, b) => a + b);
  <int>{1}.reduce((a, b) => a + b);
  const values = [1, 2];
  values.reduce((a, b) => a + b);
  list.fold<int>(0, (a, b) => a + b);
  if (list.isNotEmpty) {
    return list.reduce((a, b) => a + b);
  }
  return 0;
}

int guarded(List<int> list) => list.isEmpty ? 0 : list.reduce((a, b) => a + b);
int negated(List<int> list) {
  if (!list.isEmpty) return list.reduce((a, b) => a + b);
  return 0;
}

extension CustomReduce on String {
  String reduce(String Function(String, String) combine) => this;
}
String customExtension(String value) => value.reduce((a, b) => a + b);
