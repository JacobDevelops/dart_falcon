void f(bool a, bool b, bool c) {
  if (a) {}
  if (a && b) {}
  if (a || b || c) {}
  final x = (a && b) || c;
  final y = !(a && b);
  while (a && b && c) { break; }
  final z = a && b && [1].any((x) => a || c);
  final fallback = (a ?? b) && c;
  final bits = (1 & 2) == (3 | 4);
}
