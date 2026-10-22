import 'dart:async' as asynchronous;

typedef Completion = asynchronous.Future<void>;
typedef FutureAlias<T> = asynchronous.Future<T>;

asynchronous.Future<void> prefixed() async => null; /* expect: avoid-returning-null-for-void */
Completion alias() async => null; /* expect: avoid-returning-null-for-void */
FutureAlias<void> genericAlias() async => null; /* expect: avoid-returning-null-for-void */

class Members {
  asynchronous.Future<void> method() async {
    return null; /* expect: avoid-returning-null-for-void */
  }

  Completion get value async => null; /* expect: avoid-returning-null-for-void */
}

void nested() {
  asynchronous.Future<void> inner() async => null; /* expect: avoid-returning-null-for-void */
  inner();
}
