void f(bool a, bool b, bool? maybe) {
  if (a == true) {} /* expect: no-literal-bool-comparisons */
  if (false != a) {} /* expect: no-literal-bool-comparisons */
  if (a && true) {} /* expect: no-literal-bool-comparisons */
  if (false || b) {} /* expect: no-literal-bool-comparisons */
  final c = a & true; /* expect: no-literal-bool-comparisons */
  final d = b ^ false; /* expect: no-literal-bool-comparisons */
  final e = (a == b) == true; /* expect: no-literal-bool-comparisons */
  print([c, d, e, maybe]);
}

bool ready() => true;

void g(Object o) {
  if (ready() == false) {} /* expect: no-literal-bool-comparisons */
  if ((o is String) != true) {} /* expect: no-literal-bool-comparisons */
}
