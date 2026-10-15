void good<T>(int x, bool flag, String s) {
  if (x > 2) {}
  if (flag) {}
  if (flag && true) {}
  while (true) {}
}

void more<T>(int x, String s) {
  do {} while (x > 0);
  for (var i = 0; i < 2; i++) {}
  for (;;) {
    break;
  }
  for (final e in [1, 2]) {
    print(e);
  }
  if (1 is T) {}
  if ('$x' == '1') {}
  if (x case 1) {}
  switch (x) {
    case 1 when x > 0:
      print(x);
    default:
  }
  final list = [if (x > 1) 1];
  print(list);
}

void nonConstantInterpolation() {
  final a = 20;
  if ('$a' == '20') {}
  const outer = 20;
  void shadowed(int outer) {
    if ('$outer' == '20') {}
  }
  shadowed(outer);
}
