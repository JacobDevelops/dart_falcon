void f() {
  double coerced = 1;
  double parenthesized = (1); /* expect: omit-local-variable-types */
  print(coerced + parenthesized);
}
