// Matching promotes the scrutinee, not just the pattern's bindings.
void matchedSubjects(dynamic d) {
  if (d case String()) print(d.length);
  switch (d) {
    case String():
      print(d.length);
    default:
      break;
  }
  final n = switch (d) { List() => d.length, _ => 0 };
  print(n);
}

// An exiting match must not pollute the unmatched path.
void exitingCase(dynamic d, Map<String, Object?> cfg) {
  if (d is! String) return;
  if (cfg case {'reset': true}) {
    d = cfg;
    return;
  }
  print(d.length);
}

void exitingElse(dynamic d) {
  if (d case String()) {
    print(d.length);
  } else {
    return;
  }
  print(d.length);
}

// A failed guard is an alternative to a failed pattern, not proof of failure.
void guardedCase(dynamic d, bool accept) {
  if (d case String() when accept) return;
  d.length; /* expect: avoid-dynamic-calls */
}

// Only collected break states leave unconditional loops.
void breakPromotes(Iterator<Object?> it) {
  dynamic token;
  while (true) {
    token = it.current;
    if (token is String) break;
    it.moveNext();
  }
  print(token.length);
  dynamic next;
  for (;;) {
    next = it.current;
    if (next is String) break;
    it.moveNext();
  }
  print(next.length);
  dynamic last;
  do {
    last = it.current;
    if (last is String) break;
    it.moveNext();
  } while (true);
  print(last.length);
}

// A conditional loop can also leave before its body promotes anything.
void conditionalExit(dynamic d, bool run) {
  while (run) {
    if (d is String) break;
    return;
  }
  d.length; /* expect: avoid-dynamic-calls */
}

// Distinct breaks still join their actual states.
void mixedBreaks(dynamic d, bool stop) {
  while (true) {
    if (d is String) break;
    if (stop) break;
  }
  d.length; /* expect: avoid-dynamic-calls */
}
