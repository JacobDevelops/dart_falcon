import 'dart:async';

Future<int> fetch() async => 1;

FutureOr<int> maybeFetch() => 1;

class Api {
  Future<void> save() async {}

  Future<int> get pending async => 1;
}

void callsFunction() {
  fetch(); /* expect: discarded-futures */
}

void callsMethod(Api api) {
  api.save(); /* expect: discarded-futures */
}

void readsGetter(Api api) {
  api.pending; /* expect: discarded-futures */
}

void callsFutureOr() {
  maybeFetch(); /* expect: discarded-futures */
}

void cascade(Api api) {
  api..save(); /* expect: discarded-futures */
}

void interpolates() {
  print('${fetch()}'); /* expect: discarded-futures */
}

void inClosure() {
  final callback = () {
    fetch(); /* expect: discarded-futures */
  };
  callback();
}

class Widget {
  void initState() {
    fetch(); /* expect: discarded-futures */
  }
}
