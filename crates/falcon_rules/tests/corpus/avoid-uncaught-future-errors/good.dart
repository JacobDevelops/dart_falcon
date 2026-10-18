import 'dart:async';

Future<int> start() async => 1;
Future<void> gap() async {}

Future<void> immediatelyAwaited() async {
  try {
    final pending = start();
    await pending;
    await gap();
    await pending;
  } catch (_) {}
}

Future<void> initializerHandler() async {
  try {
    final pending = start().catchError((Object error) => 0);
    await gap();
    await pending;
  } catch (_) {}
}

Future<void> typedHandler() async {
  try {
    final pending = start().onError<Exception>((error, stack) => 0);
    await gap();
    await pending;
  } catch (_) {}
}

Future<void> thenHandler() async {
  try {
    final pending = start().then((value) => value, onError: (Object error) => 0);
    await gap();
    await pending;
  } catch (_) {}
}

Future<void> separateHandler() async {
  try {
    final pending = start();
    pending.catchError((Object error) => 0);
    await gap();
    await pending;
  } catch (_) {}
}

Future<void> handlerInitializer() async {
  try {
    final pending = start();
    final handled = pending.catchError((Object error) => 0);
    await gap();
    await pending;
    await handled;
  } catch (_) {}
}

Future<void> dynamicType(dynamic Function() create) async {
  try {
    final pending = create();
    await gap();
    await pending;
  } catch (_) {}
}

Future<void> noCatch() async {
  final pending = start();
  await gap();
  await pending;
}

Future<void> finallyOnly() async {
  try {
    final pending = start();
    await gap();
    await pending;
  } finally {}
}

Future<void> discarded() async {
  try {
    final pending = start();
    await gap();
  } catch (_) {}
}

Future<void> nestedFunction() async {
  try {
    final pending = start();
    Future<void> later() async {
      await gap();
      await pending;
    }
    await pending;
  } catch (_) {}
}

Future<void> shadowed() async {
  try {
    final pending = start();
    {
      final pending = start();
      await pending;
    }
    await pending;
  } catch (_) {}
}

Future<void> reassigned() async {
  try {
    var pending = start();
    await gap();
    pending = start();
    await pending;
  } catch (_) {}
}

Future<void> aliased() async {
  try {
    final pending = start();
    final alias = pending;
    alias.catchError((Object error) => 0);
    await gap();
    await pending;
  } catch (_) {}
}

Future<void> lazyInitialization() async {
  try {
    late final pending = start();
    await gap();
    await pending;
  } catch (_) {}
}

Future<void> multipleDeclarators() async {
  try {
    Future<int> first = start(), second = start();
    await gap();
    await first;
    await second;
  } catch (_) {}
}

Future<void> aggregateAwait() async {
  try {
    final first = start();
    final second = start();
    await Future.wait([first, second]);
    await first;
    await second;
  } catch (_) {}
}

Future<void> forwardingAwait() async {
  try {
    final pending = start();
    await pending.timeout(const Duration(seconds: 1));
    await pending;
  } catch (_) {}
}

Future<void> compositeHandlerStatement() async {
  try {
    final pending = start();
    pending.then((value) => value).catchError((Object error) => 0);
    await gap();
    await pending;
  } catch (_) {}
}

Future<void> compositeHandlerInitializer() async {
  try {
    final pending = start();
    final handled = pending.then((value) => value).catchError((Object error) => 0);
    await gap();
    await pending;
    await handled;
  } catch (_) {}
}

FutureOr<int> maybeStart() => 1;
Future<void> futureOrIsNotProof() async {
  try {
    final pending = maybeStart();
    await gap();
    await pending;
  } catch (_) {}
}

Future<void> conditionalGap(bool condition) async {
  try {
    final pending = start();
    if (condition) {
      await gap();
    }
    await pending;
  } catch (_) {}
}

Future<void> loopBoundary(List<int> values) async {
  try {
    final pending = start();
    for (final value in values) {
      await gap();
    }
    await pending;
  } catch (_) {}
}
