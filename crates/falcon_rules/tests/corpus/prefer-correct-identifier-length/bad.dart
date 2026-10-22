// Bad: too-short variable declarations, accessor names and enum constants.

void example() {
  var a = compute(); /* expect: prefer-correct-identifier-length, msg: "The `a` identifier is 1 characters long. It's recommended to increase it up to 3 chars long." */
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

// A Dart 3 destructuring declaration declares variables, so every name the
// pattern binds is length-checked — records, lists, and nested patterns alike.
void patternDeclarations(List<int> items) {
  final (a, b) = (1, 2); /* expect: prefer-correct-identifier-length */ /* expect: prefer-correct-identifier-length */
  final [c, d] = items; /* expect: prefer-correct-identifier-length */ /* expect: prefer-correct-identifier-length */
  final (e, (f, _)) = (1, (2, 3)); /* expect: prefer-correct-identifier-length */ /* expect: prefer-correct-identifier-length */
  print([a, b, c, d, e, f]);
}

// A declaration inside a labeled block or a closure body is reached too.
void nestedDeclarations() {
  lbl: {
    final gg = 1; /* expect: prefer-correct-identifier-length */
    print(gg);
  }
  final callback = () {
    final hh = 2; /* expect: prefer-correct-identifier-length */
    return hh;
  };
  print(callback);
}

final topCallback = () {
  final aa = 1; /* expect: prefer-correct-identifier-length */
  return aa;
};

class ClosureDeclarations {
  final callback = () {
    final bb = 2; /* expect: prefer-correct-identifier-length */
    return bb;
  };

  Object arrow() => () {
    final cc = 3; /* expect: prefer-correct-identifier-length */
    return cc;
  };

  int operator +(int value) {
    final dd = value; /* expect: prefer-correct-identifier-length */
    return dd;
  }
}

enum MemberDeclarations {
  value;

  int read() {
    final ee = 4; /* expect: prefer-correct-identifier-length */
    return ee;
  }
}

class InitializerDeclarations {
  final Object callback;
  InitializerDeclarations() : callback = (() {
    final ff = 5; /* expect: prefer-correct-identifier-length */
    return ff;
  });
}

List<int> collectionLoop(int count) => [
  for (var w = 0; w < count; w++) w, /* expect: prefer-correct-identifier-length */
];
