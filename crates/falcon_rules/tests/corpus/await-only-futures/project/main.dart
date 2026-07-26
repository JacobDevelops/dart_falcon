import 'dart:async' as async;
import 'barrel.dart';

async.Future<void> check(async.Future<int> future, PlainValue plain) async {
  await future;
  await plain; /* expect: await-only-futures */
}
