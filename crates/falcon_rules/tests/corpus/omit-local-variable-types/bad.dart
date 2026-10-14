class Point {
  final int x;
  Point(this.x);
}

int length(String s) => s.length;

void f(int n, String s, List<int> items, Point p, bool flag) {
  int a = 1; /* expect: omit-local-variable-types */
  String b = 'text'; /* expect: omit-local-variable-types */
  final String c = s; /* expect: omit-local-variable-types */
  Point d = Point(1); /* expect: omit-local-variable-types */
  int e = p.x; /* expect: omit-local-variable-types */
  int g = length(s); /* expect: omit-local-variable-types */
  bool h = n > 2; /* expect: omit-local-variable-types */
  List<int> i = <int>[1]; /* expect: omit-local-variable-types */
  double j = 1.5, k = 2.5; /* expect: omit-local-variable-types */
  List<int> q = []; /* expect: omit-local-variable-types */
  Set<String> r = {}; /* expect: omit-local-variable-types */
  Map<String, int> t = {'a': 1}; /* expect: omit-local-variable-types */
  for (int m = 0; m < n; m++) {} /* expect: omit-local-variable-types */
  for (int item in items) {} /* expect: omit-local-variable-types */
  print([a, b, c, d, e, g, h, i, j, k, q, r, t, flag]);
}
