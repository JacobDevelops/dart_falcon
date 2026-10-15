void numeric(int i, num n, double d, dynamic x) {
  double a = i / x; /* expect: omit-local-variable-types */
  int b = i ~/ x; /* expect: omit-local-variable-types */
  num c = i + x; /* expect: omit-local-variable-types */
  num e = n + x; /* expect: omit-local-variable-types */
  double f = d + x; /* expect: omit-local-variable-types */
  int g = i & x; /* expect: omit-local-variable-types */
  int h = i << x; /* expect: omit-local-variable-types */
  print([a, b, c, e, f, g, h]);
}
