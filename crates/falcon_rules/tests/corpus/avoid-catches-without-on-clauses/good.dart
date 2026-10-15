import 'dart:async';

void work() {}
void log(Object e) {}

void withOn() {
  try {
    work();
  } on FormatException catch (e) {
    log(e);
  }
}

void onWithoutCatch() {
  try {
    work();
  } on Exception {
    log('x');
  }
}

void rethrows() {
  try {
    work();
  } catch (e) {
    log(e);
    rethrow;
  }
}

void throwsWrapped() {
  try {
    work();
  } catch (e) {
    throw StateError('wrapped: $e');
  }
}

Future<void> futureError() {
  try {
    work();
    return Future.value();
  } catch (e) {
    return Future.error(e);
  }
}

void completer(Completer<void> completer) {
  try {
    work();
  } catch (e, s) {
    completer.completeError(e, s);
  }
}

void throwWithStack() {
  try {
    work();
  } catch (e, s) {
    Error.throwWithStackTrace(e, s);
  }
}

Never fail(Object error) => throw error;

void neverReturning() {
  try {
    work();
  } catch (e) {
    fail(e);
  }
}

void inferredCompleterInCallback(Stream<int> events) {
  final completed = Completer<Map<String, dynamic>>();
  events.listen((event) async {
    try {
      work();
    } catch (error, stack) {
      // Inferred SDK receivers forward errors just like annotated receivers.
      if (!completed.isCompleted) completed.completeError(error, stack);
    }
  });
}
