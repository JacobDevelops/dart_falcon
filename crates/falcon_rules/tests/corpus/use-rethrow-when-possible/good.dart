// Good: `rethrow`, or a throw that is not equivalent to rethrowing the caught exception.
void x() {}
void y() {}
void log(Object o) {}

void a() {
  try {
    x();
  } catch (e) {
    rethrow; // already uses rethrow
  }
}

void b() {
  try {
    x();
  } catch (e) {
    throw StateError('nope'); // throws a different object
  }
}

void c() {
  try {
    x();
  } catch (e) {
    throw Exception(e); // wraps e rather than rethrowing it
  }
}

void d() {
  try {
    x();
  } catch (e) {
    void handler() {
      throw e; // inside a nested function: rethrow would be illegal here
    }

    handler();
  }
}

void e2() {
  try {
    x();
  } catch (e) {
    try {
      y();
    } catch (e2) {
      throw e; // refers to the outer exception; rethrow is not valid here
    }
  }
}

void f() {
  try {
    x();
  } catch (e) {
    final cb = () {
      throw e; // inside a closure expression
    };
    cb();
  }
}
