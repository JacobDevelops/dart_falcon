int sum(int a, int b) => a + b;

void unsafe(List<int> list, Set<int> set, Iterable<int> iterable, List<int>? nullable) {
  list.reduce(sum); /* expect: avoid-unsafe-reduce */
  set.reduce((a, b) => a + b); /* expect: avoid-unsafe-reduce */
  iterable.reduce(sum); /* expect: avoid-unsafe-reduce */
  nullable?.reduce(sum); /* expect: avoid-unsafe-reduce */
  <int>[].reduce(sum); /* expect: avoid-unsafe-reduce */
  list..reduce(sum); /* expect: avoid-unsafe-reduce */
  if (list.isNotEmpty) {
    list.clear();
    list.reduce((a, b) => a + b); /* expect: avoid-unsafe-reduce */
  }
}

int mutatingCallback(List<int> list) {
  int Function(int, int) callback() {
    list.clear();
    return sum;
  }
  return list.isNotEmpty ? list.reduce(callback()) : 0; /* expect: avoid-unsafe-reduce */
}
