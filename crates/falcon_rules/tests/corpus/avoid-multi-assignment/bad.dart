void f(dynamic a, dynamic b, dynamic c, dynamic d) {
  a = b = 1; /* expect: avoid-multi-assignment */
  a = (b = 2); /* expect: avoid-multi-assignment */
  a = b = c = 3; /* expect: avoid-multi-assignment */
  a.value = b.value = 4; /* expect: avoid-multi-assignment */
  a += b = 5; /* expect: avoid-multi-assignment */
  d..value = b = 6; /* expect: avoid-multi-assignment */
}
