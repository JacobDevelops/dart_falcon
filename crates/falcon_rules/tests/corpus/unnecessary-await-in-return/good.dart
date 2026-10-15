Future<int> fetch() async => 1;

Future<String> fetchName() async => 'name';

Future<int> noAwait() async => fetch();

Future<int> plainReturn() async {
  return fetch();
}

Future<int> awaitInTry() async {
  try {
    return await fetch();
  } catch (e) {
    return 0;
  }
}

Future<int> awaitInFinally() async {
  try {
    print('x');
  } finally {
    return await fetch();
  }
}

Future<String> mismatch() async {
  return '${await fetch()}';
}

Future<void> used() async {
  final value = await fetch();
  print(value);
}

Future<int> awaitsNonFuture() async {
  return await 1;
}

Future<num> transformed() async {
  return (await fetch()) + 1;
}

void inferredClosureInTry() {
  final callback = () async {
    try {
      return await fetch();
    } catch (_) {
      return 0;
    }
  };
  callback();
}

inferredFunction() async => await fetch();

class UntypedDeclarations {
  method() async => await fetch();
  get value async => await fetch();
}

Future legacy() async => 1;
void register(Future<int> Function() callback) {}
void typedClosure() {
  register(() async => await legacy());
}

Future<Future<int>> nested() async => fetch();
void flattenedClosure() {
  final callback = () async => await nested();
  callback();
}
