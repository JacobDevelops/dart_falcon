int compute() => 3;
void example(bool enabled) {
  final count = 3;
  if (enabled) { print(count); } else { print(count); }
  final effectful = compute();
  if (enabled) { print(effectful); }
  var mutable = 1;
  if (enabled) { mutable++; print(mutable); }
  late final deferred = 3;
  if (enabled) { print(deferred); }
  final condition = true;
  if (condition) { print(condition); }
  final escaped = 3;
  if (enabled) { print(escaped); }
  print(escaped);
  final captured = 3;
  if (enabled) { final callback = () => captured; print(callback); }
  final first = 1, second = 2;
  if (enabled) { print(first); print(second); }
  final values = [1, 2];
  if (enabled) { print(values); }
}
void constantPattern(bool condition, int value) {
  const limit = 3;
  if (condition) { print(limit); }
  switch (value) {
    case limit: break;
    default: break;
  }
}
void ifCasePattern(bool condition, int value) {
  const limit = 3;
  if (condition) { print(limit); }
  if (value case limit) { print('matched'); }
}
