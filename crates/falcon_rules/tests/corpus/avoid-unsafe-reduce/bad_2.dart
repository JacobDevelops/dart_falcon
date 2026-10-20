void deferredInitializer(List<int> values) {
  if (values.isNotEmpty) {
    late final sum = values.reduce((a, b) => a + b); /* expect: avoid-unsafe-reduce */
    values.clear();
    print(sum);
  }
}
