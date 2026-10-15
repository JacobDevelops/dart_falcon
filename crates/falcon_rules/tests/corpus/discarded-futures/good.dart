import 'dart:async';
import 'package:meta/meta.dart' show awaitNotRequired;
import 'package:meta/meta.dart' as meta;

Future<int> fetch() async => 1;

class Api {
  Future<void> save() async {}
}

Future<void> asyncBody() async {
  fetch();
  await fetch();
}

void assigned() {
  final future = fetch();
  future.ignore();
}

Future<int> returned() {
  return fetch();
}

void unawaitedCall() {
  unawaited(fetch());
}

void delayed() {
  Future.delayed(Duration.zero, () => 1);
}

void putIfAbsent(Map<String, Future<int>> cache) {
  cache.putIfAbsent('key', fetch);
}

Future<int> arrow() => fetch();

void syncValue() {
  print(1);
}

void assignment(Api api) {
  Future<void> pending;
  pending = api.save();
  pending.ignore();
}

void annotatedLocalFunctions() {
  @awaitNotRequired
  Future<void> handleError() async {}
  handleError();
  print('${handleError()}');

  @meta.awaitNotRequired
  Future<void> prefixed() async {}
  prefixed();
}
