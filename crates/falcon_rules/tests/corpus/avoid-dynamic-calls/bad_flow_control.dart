// Closures keep promotions of variables their executable never writes.
void closureKeeps(dynamic d, List<int> xs) {
  if (d is! String) return;
  xs.forEach((_) => print(d.length));
  void local() => print(d.length);
  local();
}

// A write anywhere in the executable may precede the closure's call.
void closureLaterWrite(dynamic d, dynamic other, List<int> xs) {
  if (d is! String) return;
  xs.forEach((_) => print(d.length)); /* expect: avoid-dynamic-calls */
  d = other;
}

// `finally` forgets only what the `try` writes, and the promotions after the
// statement are those after the body.
void finallyKeeps(dynamic d) {
  if (d is! String) return;
  try {
    d.length;
  } finally {
    d.isEmpty;
  }
  d.length;
}

void finallyAfterWrite(dynamic d, dynamic other) {
  if (d is! String) return;
  try {
    d = other;
  } finally {
    d.length; /* expect: avoid-dynamic-calls */
  }
  d.length; /* expect: avoid-dynamic-calls */
}

// Switches keep promotions of variables their cases never write.
void switchKeeps(dynamic d, int k) {
  if (d is! String) return;
  switch (k) {
    case 0:
      print(d.length);
  }
  d.length;
  final n = switch (k) {
    0 => d.length,
    _ => 1,
  };
  print(n);
  d.isEmpty;
}

void switchCaseWrite(dynamic d, dynamic other, int k) {
  if (d is! String) return;
  switch (k) {
    case 0:
      d = other;
  }
  d.length; /* expect: avoid-dynamic-calls */
}

// `continue label;` may re-enter a labeled case after the switch's writes.
void switchContinue(dynamic d, dynamic other, int k) {
  if (d is! String) return;
  switch (k) {
    again:
    case 0:
      d.length; /* expect: avoid-dynamic-calls */
    case 1:
      d = other;
      continue again;
  }
}

// A pattern assignment writes only its own variables.
void patternAssignment(dynamic d, dynamic other, int a, int b) {
  if (d is! String) return;
  (a, b) = (b, a);
  d.length;
  (d, a) = (other, a);
  d.length;
}
