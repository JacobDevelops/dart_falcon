int compute(List<int> items) {
  final total = items.length;
  const limit = 10;
  var count = 0;
  count += total;
  var index = 0;
  index++;
  int? lazy;
  lazy = count;
  var a = 1, b = 2;
  b = a;
  for (var i = 0; i < limit; i++) {
    count += i;
  }
  var _ = compute(items);
  return count + (lazy ?? 0) + index + b;
}

void patterns((int, int) pair, Object o, List<(int, String)> rows) {
  final (left, right) = pair;
  var (first, second) = pair;
  first = second;
  var (_, _) = pair;
  print(left + right + first);
  for (final (n, s) in rows) {
    print('$n$s');
  }
  for (var (n, s) in rows) {
    n++;
    print('$n$s');
  }
  if (o case final int value) {
    print(value);
  }
  if (o case int value) {
    value = 2;
    print(value);
  }
  final label = switch (o) {
    int v => '$v',
    _ => '',
  };
  print(label);
}

void closures() {
  var captured = 1;
  void inner() {
    captured = 2;
  }

  inner();
  print(captured);
  var swapped = 1;
  var other = 2;
  (swapped, other) = (other, swapped);
  print(swapped + other);
  var loop = 0;
  for (loop in [1, 2, 3]) {
    print(loop);
  }
}
