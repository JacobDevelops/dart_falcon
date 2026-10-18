import 'dart:async';

Future<int> start() async => 1;
Future<void> gap() async {}

Future<void> inferredLocal() async {
  try {
    final pending = start();
    await gap();
    await pending; /* expect: avoid-uncaught-future-errors */
  } catch (_) {}
}

Future<void> explicitLocal() async {
  try {
    Future<int> pending = start();
    await gap();
    await pending; /* expect: avoid-uncaught-future-errors */
  } catch (_) {}
}

Future<void> parenthesized() async {
  try {
    final pending = (start());
    await (gap());
    await (pending); /* expect: avoid-uncaught-future-errors */
  } catch (_) {}
}

Future<void> initializerGap() async {
  try {
    final pending = start();
    final result = await start();
    await pending; /* expect: avoid-uncaught-future-errors */
  } catch (_) {}
}

Future<int> returnAwait() async {
  try {
    final pending = start();
    await gap();
    return await pending; /* expect: avoid-uncaught-future-errors */
  } catch (_) {
    return 0;
  }
}

Future<void> multiplePending() async {
  try {
    final first = start();
    final second = start();
    await gap();
    await first; /* expect: avoid-uncaught-future-errors */
    await second; /* expect: avoid-uncaught-future-errors */
  } catch (_) {}
}

Future<void> lateHandler() async {
  try {
    final pending = start();
    await gap();
    pending.catchError((Object error) => 0);
    await pending; /* expect: avoid-uncaught-future-errors */
  } catch (_) {}
}

Future<void> laterChainCanFail() async {
  try {
    final pending = start().catchError((Object error) => 0).timeout(const Duration(seconds: 1));
    await gap();
    await pending; /* expect: avoid-uncaught-future-errors */
  } catch (_) {}
}

Future<int> withRetry(Future<int> pending) => pending;

Future<void> uncalledClosureCannotConsume() async {
  try {
    final pending = start();
    final retry = () async => await withRetry(pending);
    await gap();
    await pending; /* expect: avoid-uncaught-future-errors */
  } catch (_) {}
}
