// Bad: too-short variable declarations, accessor names and enum constants.

void example() {
  var a = compute(); /* expect: prefer-correct-identifier-length */
  final xy = a; /* expect: prefer-correct-identifier-length */
  print(xy);
}

class Processor {
  int m = 0; /* expect: prefer-correct-identifier-length */

  String get n => ''; /* expect: prefer-correct-identifier-length */

  set v(String value) {} /* expect: prefer-correct-identifier-length */
}

// A C-style for-loop counter is a variable declaration, so it is checked.
void loop(List<int> items) {
  for (var i = 0; i < items.length; i++) { /* expect: prefer-correct-identifier-length */
    print(items[i]);
  }
}

enum Size { s, m, l } /* expect: prefer-correct-identifier-length */ /* expect: prefer-correct-identifier-length */ /* expect: prefer-correct-identifier-length */
