import 'dart:async';
import 'good_shadow.dart' show completeError;

abstract class Forwarding<T> implements Completer<T> {
  void forward(Object error) {
    completeError(error);
  }
}
