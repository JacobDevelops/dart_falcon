import 'dart:async';

void accept<T>(T value) {}
void acceptFuture<T>(Future<T> value) {}
void acceptFutureOr<T>(FutureOr<T> value) {}
void acceptCallback<T>(T Function() callback) {}
int makeInt() => 1;

class Box<T> {
  Box(T value);
  T field;
  void setValue(T value) {}
}

void bad(Future<int> integerFuture) {
  accept<void>(1); /* expect: void-checks */
  accept<void>(makeInt()); /* expect: void-checks */
  acceptFutureOr<void>('text'); /* expect: void-checks */
  Box<void>(1); /* expect: void-checks */
  Box<void> box = Box<void>(null);
  box.setValue(1); /* expect: void-checks */
  box.field = 1; /* expect: void-checks */
  List<void> list = [];
  list.add(1); /* expect: void-checks */
  list[0] = 1; /* expect: void-checks */
}

// Analyzer 3.13.5: adv-AdvB/repro/vc_local.dart and vc_misc.dart.
void voidLocal(dynamic value) {
  void local;
  local = 1; /* expect: void-checks */
  local = value; /* expect: void-checks */
}
void voidPatterns(void parameter) {
  [parameter] = <int>[7]; /* expect: void-checks */
  (parameter, _) = (1, 2); /* expect: void-checks */
}
class VoidLocations<T> {
  set value(T value) {}
  static void accept(void value) {}
  set dynamicReturn(dynamic value) {
    return value; /* expect: void-checks */
  }
}
extension VoidExtension on int {
  void accept(void value) {}
}
void voidCalls(void Function(void) callback, dynamic value) {
  VoidLocations<void>().value = 1; /* expect: void-checks */
  VoidLocations.accept(1); /* expect: void-checks */
  callback.call(1); /* expect: void-checks */
  VoidExtension(1).accept(value); /* expect: void-checks */
}
