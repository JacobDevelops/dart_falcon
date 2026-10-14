var a = 1, b = 2; /* expect: avoid-multiple-declarations-per-line */

int x = 0, y = 0, z = 0; /* expect: avoid-multiple-declarations-per-line */

class C {
  final int p = 1, q = 2; /* expect: avoid-multiple-declarations-per-line */
  static String? s, t; /* expect: avoid-multiple-declarations-per-line */
}

void f() {
  var i = 0, j = 1; /* expect: avoid-multiple-declarations-per-line */
  late final String first,
      second; /* expect: avoid-multiple-declarations-per-line */
  first = 'a';
  second = 'b';
  print('$i $j $first $second');
}
