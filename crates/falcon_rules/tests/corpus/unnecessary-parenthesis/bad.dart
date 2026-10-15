class Point {
  final int x;
  final int y;
  Point(this.x, int y) : y = (y); /* expect: unnecessary-parenthesis */
}

int value() => 1;

void f(int a, int b, bool c, String s, List<int> list, int? n) {
  a = (b); /* expect: unnecessary-parenthesis */
  print((a)); /* expect: unnecessary-parenthesis */
  print((a + b)); /* expect: unnecessary-parenthesis */
  if ((c)) {} /* expect: unnecessary-parenthesis */
  while ((c)) {} /* expect: unnecessary-parenthesis */
  print('${(a)}'); /* expect: unnecessary-parenthesis */
  var x = (1); /* expect: unnecessary-parenthesis */
  var y = ('text'); /* expect: unnecessary-parenthesis */
  var z = (a).isEven; /* expect: unnecessary-parenthesis */
  var w = (s).length; /* expect: unnecessary-parenthesis */
  var v = a + ((b)); /* expect: unnecessary-parenthesis */
  var u = (value()); /* expect: unnecessary-parenthesis */
  var t = (list[0]); /* expect: unnecessary-parenthesis */
  switch ((a)) { /* expect: unnecessary-parenthesis */
    default:
  }
  var r = (n?.isEven) ?? false; /* expect: unnecessary-parenthesis */
  print([x, y, z, w, v, u, t, r]);
}

int g(int a) {
  return (a); /* expect: unnecessary-parenthesis */
}

int h(int a) => (a); /* expect: unnecessary-parenthesis */

void literalTarget() {
  (1.5).floor(); /* expect: unnecessary-parenthesis */
  print(!(!true)); /* expect: unnecessary-parenthesis */
}

void cascadeClosure(List<int> list) {
  list
    ..forEach((e) {
      var even = (e.isEven); /* expect: unnecessary-parenthesis */
      print(even);
    });
}

// Only the expression parentheses are reported, not wildcard-pattern grouping.
int groupedSwitch(Object value) => switch ((value)) { /* expect: unnecessary-parenthesis */
  (_) => (1), /* expect: unnecessary-parenthesis */
};

// A declaration pattern's grouping is not the initializer's expression grouping.
void patternDeclaration() {
  var (_) = ('a'); /* expect: unnecessary-parenthesis */
}
