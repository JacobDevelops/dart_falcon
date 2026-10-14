import 'dart:async';

void work() {}
void log(Object e) {}

void bareCatch() {
  try {
    work();
  } catch (e) { /* expect: avoid-catches-without-on-clauses */
    log(e);
  }
}

void bareCatchWithStack() {
  try {
    work();
  } catch (e, s) { /* expect: avoid-catches-without-on-clauses */
    log(s);
  }
}

void throwsSomethingElse() {
  try {
    work();
  } catch (e) { /* expect: avoid-catches-without-on-clauses */
    throw StateError('failed');
  }
}

void rethrowAfterNestedCatch() {
  try {
    work();
  } catch (e) { /* expect: avoid-catches-without-on-clauses */
    try {
      work();
    } on FormatException {
      log(e);
    }
    rethrow;
  }
}

void ignoresException() {
  try {
    work();
  } catch (e) { /* expect: avoid-catches-without-on-clauses */
    print('failed');
  }
}

void completesWithOtherError(Completer<void> completer) {
  try {
    work();
  } catch (e) { /* expect: avoid-catches-without-on-clauses */
    completer.completeError(StateError('other'));
  }
}
