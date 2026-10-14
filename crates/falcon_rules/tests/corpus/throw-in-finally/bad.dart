class Ex implements Exception {}

void doThing() {}

void direct() {
  try {
    doThing();
  } finally {
    throw Ex(); /* expect: throw-in-finally */
  }
}

void afterCatch() {
  try {
    doThing();
  } catch (e) {
    print(e);
  } finally {
    throw Ex(); /* expect: throw-in-finally */
  }
}

void conditional(bool c) {
  try {
    doThing();
  } finally {
    if (c) {
      throw StateError('bad'); /* expect: throw-in-finally */
    }
  }
}

void asExpression(int? value) {
  try {
    doThing();
  } finally {
    final v = value ?? (throw ArgumentError()); /* expect: throw-in-finally */
    print(v);
  }
}

void inLoop(List<int> values) {
  try {
    doThing();
  } finally {
    for (final value in values) {
      if (value < 0) throw RangeError(value); /* expect: throw-in-finally */
    }
  }
}

void nestedFinally() {
  try {
    doThing();
  } finally {
    try {
      doThing();
    } finally {
      throw Ex(); /* expect: throw-in-finally */
    }
  }
}

void closureWithOwnTry() {
  try {
    doThing();
  } finally {
    void cleanup() {
      try {
        doThing();
      } finally {
        throw Ex(); /* expect: throw-in-finally */
      }
    }

    cleanup();
  }
}
