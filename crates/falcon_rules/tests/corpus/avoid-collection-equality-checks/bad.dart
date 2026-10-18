void compare(List<int> a, List<int> b, Set<int> c, Set<int> d, Map<String, int> e, Map<String, int> f) {
  print(a == b); /* expect: avoid-collection-equality-checks */
  print(a != b); /* expect: avoid-collection-equality-checks */
  print(c == d); /* expect: avoid-collection-equality-checks */
  print(e != f); /* expect: avoid-collection-equality-checks */
  print(a == const [1]); /* expect: avoid-collection-equality-checks */
  print(<int>[1] == <int>[2]); /* expect: avoid-collection-equality-checks */
}
