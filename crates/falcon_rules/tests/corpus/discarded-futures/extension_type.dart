import 'dart:async';

abstract class Pending {
  Future<void> close();
}

extension type PendingHandle(Pending pending) implements Pending {}

void closeHandle(PendingHandle handle) {
  handle.close(); /* expect: discarded-futures */
}
