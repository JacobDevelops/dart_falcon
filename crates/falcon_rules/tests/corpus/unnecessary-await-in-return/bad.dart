import 'dart:async';

Future<int> fetch() async => 1;

Future<String> fetchName() async => 'name';

Future<int> arrow() async => await fetch(); /* expect: unnecessary-await-in-return */

Future<int> block() async {
  return await fetch(); /* expect: unnecessary-await-in-return */
}

Future<Object> wider() async {
  return await fetchName(); /* expect: unnecessary-await-in-return */
}

Future<dynamic> dynamicResult() async => await fetch(); /* expect: unnecessary-await-in-return */

class Repository {
  Future<int> load() async {
    return await fetch(); /* expect: unnecessary-await-in-return */
  }

  Future<int> get count async => await fetch(); /* expect: unnecessary-await-in-return */
}

void outer() {
  Future<int> local() async {
    return await fetch(); /* expect: unnecessary-await-in-return */
  }

  local();
}

Future<int> afterTry() async {
  try {
    print('x');
  } finally {
    print('y');
  }
  return await fetch(); /* expect: unnecessary-await-in-return */
}

void inferredClosures() {
  final arrow = () async => await fetch(); /* expect: unnecessary-await-in-return */
  final block = () async {
    return await fetch(); /* expect: unnecessary-await-in-return */
  };
  arrow();
  block();
}


Future<List<int>> fetchList() async => <int>[1];
Future<List<num>> covariantFuture() async => await fetchList(); /* expect: unnecessary-await-in-return */

Future<int> nestedClosureInTry() async {
  try {
    final callback = () async => await fetch(); /* expect: unnecessary-await-in-return */
    return callback();
  } catch (_) {
    return 0;
  }
}

Future<Never> neverCompletes() => Future<Never>.error('failure');
Future<int> awaitsNever() async => await neverCompletes(); /* expect: unnecessary-await-in-return */

class AuthResult {}
class AuthSuccess extends AuthResult {}
void registerAuth(Future<AuthResult> Function() callback) {}
void subtypeClosure(Future<AuthSuccess> result) {
  registerAuth(() async => await result); /* expect: unnecessary-await-in-return */
}

void registerPreparation(FutureOr<void> Function() callback) {}
void futureOrClosure(Future<int> result) {
  registerPreparation(() async => await result); /* expect: unnecessary-await-in-return */
}
