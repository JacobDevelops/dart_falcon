int readCount(Map<String, int> counts, List<int> xs) {
  final a = counts['key']!;
  final b = xs[0]!; /* expect: avoid-non-null-assertion */
  return a + b;
}
