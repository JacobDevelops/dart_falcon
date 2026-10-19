import 'dart:async';

void constructors() {
  Completer<int>().completeError('ordinary'); /* expect: avoid-missing-completer-stack-trace */
  new Completer<int>().completeError('explicit'); /* expect: avoid-missing-completer-stack-trace */
  final local = Completer<int>();
  local.completeError('inferred'); /* expect: avoid-missing-completer-stack-trace */
}

abstract class Forwarding<T> implements Completer<T> {
  void fail(Object error) {
    completeError(error); /* expect: avoid-missing-completer-stack-trace */
  }
}
