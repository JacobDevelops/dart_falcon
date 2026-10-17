// Bad: re-throwing the caught exception with `throw e` instead of `rethrow`.
bool cond = true;
void x() {}
void log(Object o) {}

void a() {
  try {
    x();
  } catch (e) {
    throw e; /* expect: use-rethrow-when-possible */
  }
}

void b() {
  try {
    x();
  } catch (e) {
    log(e);
    throw e; /* expect: use-rethrow-when-possible */
  }
}

void c() {
  try {
    x();
  } catch (e) {
    if (cond) {
      throw e; /* expect: use-rethrow-when-possible */
    }
  }
}

void d() {
  try {
    x();
  } on Exception catch (e) {
    throw e; /* expect: use-rethrow-when-possible */
  }
}

void e2() {
  try {
    x();
  } catch (err, st) {
    log(st);
    throw err; /* expect: use-rethrow-when-possible */
  }
}

void nestedCatchInsideFunctionBoundaries() {
  try {
    x();
  } catch (outer) {
    final callback = () {
      try {
        x();
      } catch (inner) {
        throw inner; /* expect: use-rethrow-when-possible */
      }
    };
    void local() {
      try {
        x();
      } catch (inner) {
        throw inner; /* expect: use-rethrow-when-possible */
      }
    }
    callback();
    local();
    throw outer; /* expect: use-rethrow-when-possible */
  }
}
