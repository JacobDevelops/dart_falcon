import 'dart:async';
import 'package:meta/meta.dart' show awaitNotRequired;
import 'package:meta/meta.dart' as meta;

Future<int> fetch() async => 1;

class Api {
  Future<void> save() async {}
}

void syncBody() {
  fetch();
}

Future<void> awaited() async {
  await fetch();
}

Future<void> marked() async {
  unawaited(fetch());
}

Future<void> assigned() async {
  final future = fetch();
  await future;
}

Future<int> returned() async {
  return fetch();
}

Future<void> delayed() async {
  Future.delayed(Duration.zero, () => 1);
}

Future<void> putIfAbsent(Map<String, Future<int>> cache) async {
  cache.putIfAbsent('key', fetch);
}

Future<void> assignment(Api api) async {
  Future<void> pending;
  pending = api.save();
  await pending;
}

FutureOr<int> maybeFetchOr() => 1;

// Like the official rule, `FutureOr` results are not reported in async bodies.
Future<void> callsFutureOr() async {
  maybeFetchOr();
}

Future<void> annotatedLocalFunctions() async {
  @awaitNotRequired
  Future<void> handleError() async {}
  handleError();
  print('${handleError()}');

  @meta.awaitNotRequired
  Future<void> prefixed() async {}
  prefixed();
}
