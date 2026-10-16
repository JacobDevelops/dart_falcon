import 'dart:async';

class Target {}
bool predicate<T>(bool Function(T) callback) => true;
void concrete(bool Function(Object) callback) {}
void siblings<T>(T Function() create, void Function(T) dispose) {}
void reverseSiblings<T>(void Function(T) dispose, T Function() create) {}
void typedSiblings<T>(void Function(T) first, void Function(T) second) {}
void consumeNum(num value) {}
class CallbackBox<T> {
  CallbackBox({required T Function() create, required void Function(T) dispose});
}
void namedSiblings<T>({
  required T Function() create,
  required void Function(T) dispose,
}) {}

void callbacks(Future<Object?> future) {
  future.onError((error, stack) {
    error as Target; /* expect: cast-nullable-to-non-nullable */
    return null;
  });
  predicate((trace) {
    trace as StackTrace; /* expect: cast-nullable-to-non-nullable */
    return true;
  });
  predicate<Object>((value) {
    value as Target;
    return true;
  });
  concrete((value) { value as Target; return true; });
  siblings(() => Target(), (value) { value as Target; });
  namedSiblings(
    create: () => Target(),
    dispose: (value) { value as Target; },
  );
  reverseSiblings((value) { value as Target; }, () => Target());
  namedSiblings(
    dispose: (value) { value as Target; },
    create: () => Target(),
  );
  CallbackBox(
    dispose: (value) { value as Target; },
    create: () => Target(),
  );
  typedSiblings((num value) {}, (value) {
    value as int; /* expect: cast-nullable-to-non-nullable */
  });
  typedSiblings((value) {
    value as int; /* expect: cast-nullable-to-non-nullable */
  }, (num value) {});
  typedSiblings(consumeNum, (value) {
    value as int; /* expect: cast-nullable-to-non-nullable */
  });
  typedSiblings((value) {
    value as int; /* expect: cast-nullable-to-non-nullable */
  }, consumeNum);
  final noContext = (value) { value as Target; };
  noContext(null);
}
