// Type parameters bounded by `Future` subtypes, directly or transitively.
extension type FutureWrapper(Future<int> value) implements Future<int> {}

Future<void> bounds<T extends Future<int>, U extends T, W extends FutureWrapper>(
  T future,
  U nested,
  W wrapped,
  T? nullable,
) async {
  await future;
  await nested;
  await wrapped;
  await nullable;
}
