import 'dart:async';

class Custom {
  void completeError(Object error) {}
}

void errors(Completer<int> completer, Completer<void>? nullable, dynamic unknown, StackTrace trace) {
  completer.completeError('first', trace);
  completer.completeError('second', StackTrace.current);
  nullable?.completeError('third', trace);
  completer..completeError('fourth', trace);
  Custom().completeError('custom');
  unknown.completeError('unknown');
  completer.complete(1);
}

abstract class Overriding<T> implements Completer<T> {
  void completeError(Object error, [StackTrace? stackTrace]) {}
  void fail(Object error) {
    completeError(error);
  }
}

extension CustomCompleterErrors<T> on Completer<T> {
  void completeError(Object error) {}
  void failWithCustomHandler(Object error) {
    completeError(error);
  }
}
