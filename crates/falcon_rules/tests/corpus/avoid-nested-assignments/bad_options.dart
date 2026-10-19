void f(dynamic x) {
  consume(x = 1); /* expect: avoid-nested-assignments */
}
