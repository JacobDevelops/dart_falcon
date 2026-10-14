class Ex implements Exception {}

void doThing() {}

void inTry() {
  try {
    throw Ex();
  } finally {
    print('cleanup');
  }
}

void inCatch() {
  try {
    doThing();
  } catch (e) {
    throw Ex();
  } finally {
    print('cleanup');
  }
}

void rethrowInCatch() {
  try {
    doThing();
  } catch (e) {
    rethrow;
  } finally {
    print('cleanup');
  }
}

// A closure defined in the finally does not throw from it.
void closure() {
  try {
    doThing();
  } finally {
    final fail = () => throw Ex();
    void local() {
      throw Ex();
    }

    print([fail, local]);
  }
}

// The innermost enclosing try owns the throw.
void nestedTryBody() {
  try {
    doThing();
  } finally {
    try {
      throw Ex();
    } catch (e) {
      print(e);
    }
  }
}

void nestedTryCatch() {
  try {
    doThing();
  } finally {
    try {
      doThing();
    } catch (e) {
      throw Ex();
    }
  }
}

void outside() {
  try {
    doThing();
  } finally {
    print('cleanup');
  }
  throw Ex();
}
