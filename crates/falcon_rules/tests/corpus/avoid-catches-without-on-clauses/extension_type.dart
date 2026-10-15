import 'dart:async';

extension type Completion(Completer<void> value) implements Object {
  void completeError(Object error, StackTrace stack) {
    value.completeError(error, stack);
  }
}

extension type NotCompletion(int value) {
  int completeError(Object error, StackTrace stack) {
    return value;
  }
}

void forward(Completion completer, void Function() work) {
  try {
    work();
  } catch (error, stack) {
    completer.completeError(error, stack);
  }
}

void boundedForward<T extends Completion>(T completer, void Function() work) {
  try {
    work();
  } catch (error, stack) {
    completer.completeError(error, stack);
  }
}

void notForward(NotCompletion completer, void Function() work) {
  try {
    work();
  } catch (error, stack) { /* expect: avoid-catches-without-on-clauses */
    completer.completeError(error, stack);
  }
}
