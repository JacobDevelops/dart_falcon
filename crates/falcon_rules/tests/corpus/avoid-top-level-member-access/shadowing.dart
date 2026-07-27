Object shared = 1; /* expect: avoid-top-level-member-access */

void parameter(Object shared) {
  print(shared);
}

void localVariable() {
  final shared = 2, copy = shared;
  print([shared, copy]);
}

void catchVariable() {
  try {
    work();
  } catch (shared) {
    print(shared);
  }
}

void loopVariable() {
  for (final shared in [1, 2]) {
    print(shared);
  }
  for (final (shared,) in [(1,)]) {
    print(shared);
  }
  for (var shared = 0; shared < 1; shared++) {
    print(shared);
  }
  print(shared); /* expect: avoid-top-level-member-access */
}

void patternVariable(Object value) {
  final (shared,) = (value,);
  print(shared);
  if (value case var shared) {
    print(shared);
  }
}

void closureParameter() {
  final callback = (Object shared) => shared;
  print(callback);
}

void localFunction() {
  Object shared() => 3;
  print(shared());
}

void work() {}

void implicitStatementLists(int value) {
  try {
    print(shared());
    Object shared() => 2;
  } catch (_) {
    print(shared); /* expect: avoid-top-level-member-access */
    final shared = 3;
    print(shared);
  } finally {
    print(shared());
    Object shared() => 4;
  }
  print(shared); /* expect: avoid-top-level-member-access */

  switch (value) {
    case 0:
      print(shared());
      Object shared() => 5;
      break;
    default:
      print(shared); /* expect: avoid-top-level-member-access */
  }
}
