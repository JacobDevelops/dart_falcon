var count = 0; /* expect: always-specify-types */
final String name = 'x';
const limit = 3; /* expect: always-specify-types */
List raw = <int>[]; /* expect: always-specify-types */

class Box<T> {
  final T value;
  Box(this.value);
}

class Holder {
  final items = <int>[]; /* expect: always-specify-types */
  Map lookup = <String, int>{}; /* expect: always-specify-types */
  Box box = Box<int>(1); /* expect: always-specify-types */
}

void f(x, int y) { /* expect: always-specify-types */
  var local = 1; /* expect: always-specify-types */
  final List<int> list = []; /* expect: always-specify-types */
  final Set<int> set = {}; /* expect: always-specify-types */
  final Map<String, int> map = {}; /* expect: always-specify-types */
  for (var i = 0; i < 2; i++) {} /* expect: always-specify-types */
  for (final e in list) {} /* expect: always-specify-types */
  final Box<int> b = Box(1); /* expect: always-specify-types */
  final (int a, String s) = (1, '');
  final (int c, d) = (1, 2); /* expect: always-specify-types */
  if (y case final z) {} /* expect: always-specify-types */
  final g = (v) => v; /* expect: always-specify-types */ /* expect: always-specify-types */
}

set value(v) {} /* expect: always-specify-types */
