class Combiner {
  final List<int> values;
  Combiner(this.values);
  int Function(int, int) get combine {
    values.clear();
    return (a, b) => a + b;
  }

  int sum(List<int> xs) => xs.isEmpty ? 0 : xs.reduce(combine); /* expect: avoid-unsafe-reduce */
}
