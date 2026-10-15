// A closure keeps the promotion of a variable its executable never writes.
void closureKeeps(int? x, List<int> xs) {
  if (x == null) return;
  xs.forEach((_) => print(x as int)); /* expect: avoid-unnecessary-type-casts */
}

// `finally` keeps promotions the `try` never writes, and so does the code after it.
void finallyKeeps(int? x) {
  if (x == null) return;
  try {
    print(x);
  } finally {
    print(x as int); /* expect: avoid-unnecessary-type-casts */
  }
  print(x as int); /* expect: avoid-unnecessary-type-casts */
}

// Switches keep promotions their cases never write.
void switchKeeps(int? x, int k) {
  if (x == null) return;
  switch (k) {
    case 0:
      print(x as int); /* expect: avoid-unnecessary-type-casts */
  }
  print(x as int); /* expect: avoid-unnecessary-type-casts */
}

// A pattern assignment writes only its own variables.
void patternAssignment(int? x, int a, int b) {
  if (x == null) return;
  (a, b) = (b, a);
  print(x as int); /* expect: avoid-unnecessary-type-casts */
}
