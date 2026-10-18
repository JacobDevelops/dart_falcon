void guardedInitializer(List<int> values) {
  if (values.isNotEmpty) {
    final sum = values.reduce((a, b) => a + b);
    print(sum);
  }
}
