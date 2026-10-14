import 'dart:async';

Future<int> fetch() async => 1;

FutureOr<int> maybeFetch() => 1;

class Api {
  Future<void> save() async {}

  Future<int> get pending async => 1;
}

Future<void> callsFunction() async {
  fetch(); /* expect: unawaited-futures */
}

Future<void> callsMethod(Api api) async {
  api.save(); /* expect: unawaited-futures */
}

Future<void> readsGetter(Api api) async {
  api.pending; /* expect: unawaited-futures */
}

Future<void> callsFutureOr() async {
  maybeFetch(); /* expect: unawaited-futures */
}

Future<void> cascade(Api api) async {
  api..save(); /* expect: unawaited-futures */
}

Stream<int> generator() async* {
  fetch(); /* expect: unawaited-futures */
  yield 1;
}

void syncWithAsyncClosure() {
  final callback = () async {
    fetch(); /* expect: unawaited-futures */
  };
  callback();
}
