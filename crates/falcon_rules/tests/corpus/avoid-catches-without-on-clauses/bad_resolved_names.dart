class Future<T> {
  Future.error(Object error);
}

class FlutterError {
  static void reportError(Object error) {}
}

class Error {
  static void throwWithStackTrace(Object error, StackTrace stack) {}
}

class Completer {
  void completeError(Object error) {}
}

void localFuture() {
  try {} catch (error) { /* expect: avoid-catches-without-on-clauses */
    Future<void>.error(error);
  }
}

void explicitLocalFuture() {
  try {} catch (error) { /* expect: avoid-catches-without-on-clauses */
    new Future<void>.error(error);
  }
}

void localFlutterError() {
  try {} catch (error) { /* expect: avoid-catches-without-on-clauses */
    FlutterError.reportError(error);
  }
}

void localError() {
  try {} catch (error, stack) { /* expect: avoid-catches-without-on-clauses */
    Error.throwWithStackTrace(error, stack);
  }
}

void localCompleter(Completer completer) {
  try {} catch (error) { /* expect: avoid-catches-without-on-clauses */
    completer.completeError(error);
  }
}
