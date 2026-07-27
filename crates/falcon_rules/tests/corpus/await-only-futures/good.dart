import 'dart:async';

extension type FutureBox(Future<int> value) {}

Future<int> asyncValue() async => 1;

Future<void> good<T extends Future<int>>(
  Future<int> future,
  FutureOr<int> futureOr,
  Future<int>? nullableFuture,
  dynamic anything,
  T bounded,
  FutureBox extensionValue,
) async {
  await future;
  await futureOr;
  await nullableFuture;
  await anything;
  await bounded;
  await extensionValue;
  await null;
  await asyncValue();
}

Future<void> branchBindings(bool condition, Future<int> future) async {
  if (condition)
    final future = 1;
  else
    final future = 2;
  await future;
}
