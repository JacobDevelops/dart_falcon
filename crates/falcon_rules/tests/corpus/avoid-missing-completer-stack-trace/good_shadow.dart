import 'dart:async';

void completeError(Object error) {}

abstract class Forwarding<T> implements Completer<T> {
  void forward(Object error) {
    completeError(error);
  }
}
