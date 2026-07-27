import 'dart:async';

int syncValue() => 1;

Future<void> bad(int count, String label, Object value) async {
  await count; /* expect: await-only-futures */
  await label; /* expect: await-only-futures */
  await value; /* expect: await-only-futures */
  await true; /* expect: await-only-futures */
  await syncValue(); /* expect: await-only-futures */
}

Future<void> oldStyleFormal(Future<int> callback()) async {
  await callback; /* expect: await-only-futures */
}

Future<void> scoped(Future<int> future, List<int> values) async {
  {
    final future = 1;
    await future; /* expect: await-only-futures */
  }
  await future;

  for (final value in values) {
    await value; /* expect: await-only-futures */
  }

  await (() async {
    await 1; /* expect: await-only-futures */
  })();
}
