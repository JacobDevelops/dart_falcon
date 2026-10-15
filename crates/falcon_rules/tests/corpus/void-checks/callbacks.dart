import 'dart:async';

void accept<T>(T value) {}
void oldStyle(void callback()) {}
void callback(void Function() callback) {}
int tearOff() => 1;

void callbackContexts(dynamic value, Future<void> future, Stream<int> stream) {
  oldStyle(() {});
  oldStyle(tearOff);
  callback(() => 1);
  callback(() {
    return value; /* expect: void-checks */
  });
  callback(tearOff);
  future.whenComplete(() {});
  future.whenComplete(() => 1);
  future.whenComplete(() { return value; });
  stream.listen((value) {}, onDone: () {});
  // `Future.whenComplete` needs SDK signatures; `whenDone` mirrors its
  // old-style `FutureOr<void> action()` formal.
  whenDone(() { /* expect: void-checks */
    return 1;
  });
  whenDone(() async {
    return 1;
  });
  callback(() async {
    return value;
  });
  <String, int>{}.forEach((key, value) {});
  accept<void>(tearOff); /* expect: void-checks */
  accept<void>(value); /* expect: void-checks */
  accept<FutureOr<void>>(1); /* expect: void-checks */
  accept<FutureOr<void>>(value);
}

void returnDynamic(dynamic value) {
  return value; /* expect: void-checks */
}
FutureOr<void> returnFutureOr(dynamic value) {
  return 1; /* expect: void-checks */
}
void whenDone(FutureOr<void> action()) {}
