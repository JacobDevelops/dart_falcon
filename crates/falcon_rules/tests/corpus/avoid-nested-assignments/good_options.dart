void f(dynamic x) {
  consume(x ??= 1);
  final value = (x ??= 2);
  final message = '${x ??= 3}';
  x ??= 4;
  for (x = 0; x < 2; x += 1) {}
}
