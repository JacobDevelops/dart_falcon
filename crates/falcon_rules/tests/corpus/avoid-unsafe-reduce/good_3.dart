int earlyReturn(List<int> values) {
  if (values.isEmpty) return 0;
  return values.reduce((a, b) => a + b);
}

int earlyThrow(List<int> values) {
  if (!values.isNotEmpty) {
    throw StateError('empty');
  }
  return values.reduce((a, b) => a + b);
}
