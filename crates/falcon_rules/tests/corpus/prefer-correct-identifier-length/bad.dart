// Bad: single-character identifiers (except i, j, k, n in loops)
void example() {
  var a = compute(); /* expect: prefer-correct-identifier-length */
  String s = getText(); /* expect: prefer-correct-identifier-length */
  int x = 42; /* expect: prefer-correct-identifier-length */
  List<String> l = []; /* expect: prefer-correct-identifier-length */
}

class Processor {
  String p = ''; /* expect: prefer-correct-identifier-length */
  int m = 0; /* expect: prefer-correct-identifier-length */

  void process(String d) { /* expect: prefer-correct-identifier-length */
    final r = transform(d); /* expect: prefer-correct-identifier-length */
    print(r);
  }
}

void badLoop(List<int> items) {
  for (int x = 0; x < items.length; x++) { /* expect: prefer-correct-identifier-length */
    final v = items[x]; /* expect: prefer-correct-identifier-length */
    print(v);
  }
}

String f(int q) { /* expect: prefer-correct-identifier-length */ /* expect: prefer-correct-identifier-length */
  return 'Result: $q';
}
